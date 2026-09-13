"""PUT API script metadata, derived from wrangler.toml.

The previous version hardcoded a copy of `[vars]` and three bindings inside the
shell script. That copy drifted: by 2026-09-12 it was missing six variables that
wrangler.toml declares (notifications, Telegram, Helius, Crossmint) and all four
rate-limiter bindings, while still sending four deposit variables wrangler.toml
had commented out. A fallback deploy therefore shipped a quietly different
production configuration than `wrangler deploy` would have.

Reading wrangler.toml removes that whole class of drift. What the PUT API
genuinely cannot express is reported as a warning instead of being dropped in
silence.
"""

import tomllib

# Cloudflare's PUT /workers/scripts/{name} expects environment variables as
# `plain_text` entries in `bindings` — NOT a top-level `vars` dict (that is the
# wrangler.toml shape). Sending `vars` silently drops every variable and the
# worker falls back to its hardcoded defaults.
PLAIN_TEXT = "plain_text"

# wrangler.toml table -> (API binding type, name key, {api field: toml field})
BINDING_TABLES = {
    "kv_namespaces": ("kv_namespace", "binding", {"namespace_id": "id"}),
    "d1_databases": ("d1", "binding", {"id": "database_id"}),
    "r2_buckets": ("r2_bucket", "binding", {"bucket_name": "bucket_name"}),
}


def load_config(path: str) -> dict:
    """Parse wrangler.toml."""
    with open(path, "rb") as handle:
        return tomllib.load(handle)


def _resource_bindings(config: dict) -> list[dict]:
    bindings: list[dict] = []
    for table, (api_type, name_key, fields) in BINDING_TABLES.items():
        for entry in config.get(table, []):
            binding = {"type": api_type, "name": entry[name_key]}
            for api_field, toml_field in fields.items():
                binding[api_field] = entry[toml_field]
            bindings.append(binding)
    return bindings


def _ratelimit_bindings(config: dict) -> list[dict]:
    return [
        {
            "type": "ratelimit",
            "name": entry["name"],
            "namespace_id": entry["namespace_id"],
            "simple": {
                "limit": entry["simple"]["limit"],
                "period": entry["simple"]["period"],
            },
        }
        for entry in config.get("ratelimits", [])
    ]


def unsupported_config(config: dict, include_ratelimits: bool) -> list[str]:
    """Everything wrangler.toml declares that this metadata will not carry."""
    warnings: list[str] = []
    if "durable_objects" in config:
        warnings.append(
            "durable_objects: the PUT API rejects the binding type (10021); "
            "only `wrangler deploy` can ship Durable Objects"
        )
    if config.get("ratelimits") and not include_ratelimits:
        names = ", ".join(entry["name"] for entry in config["ratelimits"])
        warnings.append(
            f"ratelimits ({names}): NOT sent. The `ratelimit` binding type is "
            "unverified against this API; the worker falls back to its in-memory "
            "limiter. Pass --include-ratelimits once verified (Issue #065 step 5)"
        )
    if "triggers" in config:
        warnings.append("triggers (cron): not part of script metadata; set separately")
    if "placement" in config:
        warnings.append("placement (smart): not part of script metadata")
    # Issue #057: the PUT API has no field for _headers rules, so this path cannot
    # apply the frontend Cache-Control policy. Assets get Cloudflare's default
    # 'max-age=0, must-revalidate' — always revalidates, never stale, just not
    # maximally cacheable. Perf-only, and only under the fallback.
    warnings.append("_headers: not representable; assets get max-age=0, must-revalidate")
    return warnings


def build_metadata(
    config: dict,
    assets_jwt: str,
    main_module: str,
    include_ratelimits: bool = False,
) -> dict:
    """Build the metadata document for PUT /workers/scripts/{name}."""
    bindings = _resource_bindings(config)
    if include_ratelimits:
        bindings.extend(_ratelimit_bindings(config))
    for name, value in config.get("vars", {}).items():
        # The API stores plain_text verbatim; a TOML bool/int would either be
        # rejected or arrive as a shape the worker cannot parse. Fail loudly.
        if not isinstance(value, str):
            raise TypeError(
                f"[vars] {name} must be a quoted string, got {type(value).__name__}"
            )
        bindings.append({"type": PLAIN_TEXT, "name": name, "text": value})

    assets = config.get("assets", {})
    return {
        "main_module": main_module,
        "compatibility_date": config["compatibility_date"],
        "compatibility_flags": config.get("compatibility_flags", []),
        "bindings": bindings,
        "assets": {
            "jwt": assets_jwt,
            "router_config": {"has_user_worker": True},
            "asset_config": {
                "not_found_handling": assets.get(
                    "not_found_handling", "single-page-application"
                )
            },
        },
    }
