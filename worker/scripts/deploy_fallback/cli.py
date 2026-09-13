"""Command line entry points for the PUT API deploy fallback.

    python3 -m deploy_fallback.cli manifest --dist DIR
    python3 -m deploy_fallback.cli upload   --dist DIR --api-base URL --script NAME
    python3 -m deploy_fallback.cli metadata --config PATH --main-module NAME

Secrets never appear in argv — `ps` is world-readable on this machine, and the
old `python3 -c` form interpolated the Cloudflare OAuth token straight into the
program text. The token is read from CLOUDFLARE_OAUTH_TOKEN and the assets JWT
from CLOUDFLARE_ASSETS_JWT.

stdout carries exactly one machine-readable value per subcommand; all progress
and every warning goes to stderr so the caller can capture stdout directly.
"""

import argparse
import json
import os
import sys

from .manifest import build_manifest
from .metadata import build_metadata, load_config, unsupported_config
from .upload import status, strip_jwt_prefix, upload_assets

OAUTH_TOKEN_ENV = "CLOUDFLARE_OAUTH_TOKEN"
ASSETS_JWT_ENV = "CLOUDFLARE_ASSETS_JWT"


def _required_env(name: str) -> str:
    value = os.environ.get(name, "")
    if not value:
        raise SystemExit(f"{name} is not set (it must be exported, never passed in argv)")
    return value


def _cmd_manifest(args: argparse.Namespace) -> None:
    print(json.dumps({"manifest": build_manifest(args.dist)}))


def _cmd_upload(args: argparse.Namespace) -> None:
    manifest = json.load(sys.stdin)["manifest"]
    print(
        upload_assets(
            api_base=args.api_base,
            script=args.script,
            oauth=_required_env(OAUTH_TOKEN_ENV),
            dist=args.dist,
            manifest=manifest,
        )
    )


def _cmd_metadata(args: argparse.Namespace) -> None:
    config = load_config(args.config)
    for warning in unsupported_config(config, args.include_ratelimits):
        status(f"   ⚠️  fallback cannot carry {warning}")
    metadata = build_metadata(
        config=config,
        # The PUT API rejects a `cfwau_`-prefixed JWT with 10021. `upload`
        # already strips it; strip again here so a hand-supplied token from
        # a manual re-run cannot fail the deploy at the last step.
        assets_jwt=strip_jwt_prefix(_required_env(ASSETS_JWT_ENV)),
        main_module=args.main_module,
        include_ratelimits=args.include_ratelimits,
    )
    print(json.dumps(metadata))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="deploy_fallback")
    sub = parser.add_subparsers(dest="command", required=True)

    manifest = sub.add_parser("manifest", help="print the asset manifest as JSON")
    manifest.add_argument("--dist", required=True)
    manifest.set_defaults(handler=_cmd_manifest)

    upload = sub.add_parser("upload", help="upload assets, print the completion JWT")
    upload.add_argument("--dist", required=True)
    upload.add_argument("--api-base", required=True)
    upload.add_argument("--script", required=True)
    upload.set_defaults(handler=_cmd_upload)

    metadata = sub.add_parser("metadata", help="print PUT script metadata as JSON")
    metadata.add_argument("--config", required=True, help="path to wrangler.toml")
    metadata.add_argument("--main-module", default="shim.js")
    metadata.add_argument(
        "--include-ratelimits",
        action="store_true",
        help="send ratelimit bindings (unverified against the PUT API; see Issue #065)",
    )
    metadata.set_defaults(handler=_cmd_metadata)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        args.handler(args)
    except RuntimeError as error:
        status(f"   ❌ {error}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
