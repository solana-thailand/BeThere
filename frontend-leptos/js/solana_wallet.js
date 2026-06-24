/**
 * Solana wallet adapter interop for BeThere deposit flow.
 *
 * Uses the Wallet Standard API to detect, connect, and transact with
 * Solana wallets (Phantom, Backpack, Solflare, etc.).
 *
 * Supports both legacy injection (window.solana) and modern injection
 * patterns (window.phantom.solana, Wallet Standard registry).
 *
 * Imported via `#[wasm_bindgen(module = "/js/solana_wallet.js")]` in Rust.
 */

/**
 * Get a list of detected Solana wallet adapter names.
 *
 * Detection order:
 * 1. Legacy adapters: window.solana (Phantom), window.phantom?.solana,
 *    window.backpack?.solana, window.solflare, window.coinbaseSolana
 * 2. Wallet Standard registry: window.navigator.wallets
 *
 * @returns {Array<string>} Array of wallet names, e.g. ["Phantom", "Backpack"]
 */
export function getDetectedWallets() {
  var wallets = [];

  // Legacy adapter detection — order matters: this drives the button order
  // in the UI. Solflare is listed first because it's the primary recommended
  // wallet for this app's deposit/claim flows.

  // Solflare: window.solflare or window.solflare.isSolflare
  if (window.solflare && window.solflare.isSolflare) {
    wallets.push("Solflare");
  }

  // Phantom: primary (window.solana.isPhantom) + secondary (window.phantom.solana)
  if (window.solana && window.solana.isPhantom) {
    wallets.push("Phantom");
  } else if (
    window.phantom &&
    window.phantom.solana &&
    window.phantom.solana.isPhantom
  ) {
    wallets.push("Phantom");
  }

  // Backpack: window.backpack.solana or window.backpack._isBackpack
  if (window.backpack && window.backpack.solana) {
    wallets.push("Backpack");
  } else if (window.backpack && window.backpack._isBackpack) {
    wallets.push("Backpack");
  }

  // Coinbase: window.coinbaseSolana or window.coinbaseSolana?.isCoinbase
  if (window.coinbaseSolana) {
    wallets.push("Coinbase");
  }

  // Generic Solana adapter (any wallet that injects window.solana without isPhantom)
  if (
    window.solana &&
    !window.solana.isPhantom &&
    window.solana.connect &&
    wallets.indexOf("Phantom") === -1
  ) {
    wallets.push("Solana");
  }

  // Wallet Standard detection (newer approach — used by most modern wallets)
  // window.navigator.wallets is the Wallet Standard registry
  if (window.navigator && window.navigator.wallets) {
    try {
      var standardWallets = window.navigator.wallets.get();
      for (var i = 0; i < standardWallets.length; i++) {
        var w = standardWallets[i];
        // Only include Solana-capable wallets not already detected
        if (
          w.chains &&
          w.chains.some(function (c) {
            return c.indexOf("solana:") === 0;
          })
        ) {
          var name = w.name || "Unknown";
          if (wallets.indexOf(name) === -1) {
            wallets.push(name);
          }
        }
      }
    } catch (e) {
      console.warn("[solana_wallet] Wallet Standard registry error:", e);
    }
  }

  console.log("[solana_wallet] Detected wallets (sync):", wallets);
  return wallets;
}

/**
 * Async version of getDetectedWallets — waits up to 3 seconds for wallet
 * extensions to inject their providers before returning.
 *
 * This is needed because modern wallets (especially Phantom) inject
 * asynchronously after page load. The sync version may return [] if
 * called too early.
 *
 * @returns {Promise<Array<string>>} Array of wallet names
 */
export async function getDetectedWalletsAsync() {
  // Check immediately first
  var wallets = getDetectedWallets();
  if (wallets.length > 0) {
    return wallets;
  }

  // No wallets detected — wait for async injection (up to 3 seconds)
  console.log(
    "[solana_wallet] No wallets found yet, waiting for async injection...",
  );
  var maxAttempts = 10;
  var delay = 300;
  for (var i = 0; i < maxAttempts; i++) {
    await new Promise(function (resolve) {
      setTimeout(resolve, delay);
    });
    wallets = getDetectedWallets();
    if (wallets.length > 0) {
      console.log(
        "[solana_wallet] Wallets detected after",
        (i + 1) * delay,
        "ms:",
        wallets,
      );
      return wallets;
    }
  }

  console.warn("[solana_wallet] No wallets detected after 3s wait");
  return [];
}

/**
 * Connect to a Solana wallet and return the public key (base58).
 *
 * First tries the synchronous path (provider already injected).
 * If the wallet is not found, waits up to 3 seconds for async injection
 * before giving up.
 *
 * @param {string} walletName - Name of the wallet to connect (e.g. "Phantom", "Backpack")
 * @returns {Promise<string|null>} Base58-encoded public key, or null on failure
 */
export async function connectWallet(walletName) {
  console.log("[solana_wallet] connectWallet called for:", walletName);

  // Guard: reject empty/falsy wallet names immediately
  // Prevents WASM passStringToWasm0 crash from empty strings
  if (!walletName || !walletName.trim()) {
    console.error("[solana_wallet] connectWallet: empty wallet name, ignoring");
    return JSON.stringify({
      __wallet_error__: true,
      code: null,
      message: "No wallet selected",
      logs: null,
    });
  }

  // Try synchronously first
  var provider = getProvider(walletName);
  if (provider) {
    return doConnect(provider, walletName);
  }

  // Provider not found yet — wallet may be injecting async.
  // Wait up to 3 seconds polling every 300ms.
  console.log(
    "[solana_wallet] Provider not found yet, waiting for async injection...",
  );
  var maxAttempts = 10;
  var delay = 300;
  for (var i = 0; i < maxAttempts; i++) {
    await sleep(delay);
    provider = getProvider(walletName);
    if (provider) {
      console.log(
        "[solana_wallet] Provider found after",
        (i + 1) * delay,
        "ms",
      );
      return doConnect(provider, walletName);
    }
  }

  console.error("[solana_wallet] Wallet not found after waiting:", walletName);
  console.error(
    "[solana_wallet] Available: window.solana=",
    !!window.solana,
    "window.phantom=",
    !!window.phantom,
    "window.solflare=",
    !!window.solflare,
    "window.backpack=",
    !!window.backpack,
  );
  return JSON.stringify({
    __wallet_error__: true,
    code: null,
    message:
      "Wallet not found. Please install " +
      walletName +
      " and refresh the page.",
    logs: null,
  });
}

/**
 * Resolve the user's PublicKey from a connect() response or the provider
 * instance itself.
 *
 * @param {*} response - value returned by provider.connect()
 * @param {*} provider - the provider instance (may carry .publicKey directly)
 * @returns {object|null} PublicKey-like object with toBase58(), or null
 */
function resolvePublicKey(response, provider) {
  // 1. Phantom/Backpack legacy: response.publicKey
  if (response && response.publicKey) {
    // Some Wallet Standard adapters nest it: response.publicKey.publicKey
    if (response.publicKey.publicKey) {
      return response.publicKey.publicKey;
    }
    return response.publicKey;
  }

  // 2. Solflare legacy: publicKey lives on the provider after connect()
  if (provider && provider.publicKey) {
    console.log("[solana_wallet] Using provider.publicKey (Solflare-style)");
    return provider.publicKey;
  }

  return null;
}

/**
 * Internal: perform the actual connect call on a provider.
 *
 * Public key resolution handles three provider conventions:
 * 1. Phantom/Backpack legacy: connect() resolves to { publicKey: PublicKey }
 * 2. Wallet Standard / nested adapters: response.publicKey.publicKey
 * 3. Solflare legacy: connect() resolves to undefined/true, and the public key
 *    is exposed at provider.publicKey after connect completes (and may be set
 *    asynchronously, hence the short poll).
 */
async function doConnect(provider, walletName) {
  try {
    console.log("[solana_wallet] Provider found, calling connect()...");
    var response = await provider.connect();
    console.log("[solana_wallet] Connect response:", response);

    var publicKey = resolvePublicKey(response, provider);

    // Solflare's legacy provider sets publicKey asynchronously in some
    // versions — poll briefly before giving up.
    if (!publicKey && provider && !provider.publicKey) {
      for (var i = 0; i < 10; i++) {
        await sleep(100);
        if (provider.publicKey) {
          publicKey = provider.publicKey;
          console.log(
            "[solana_wallet] publicKey appeared after",
            (i + 1) * 100,
            "ms",
          );
          break;
        }
      }
    }

    if (!publicKey) {
      console.error("[solana_wallet] No public key in response");
      return JSON.stringify({
        __wallet_error__: true,
        code: null,
        message: "Wallet returned no public key",
        logs: null,
      });
    }

    var pkBase58 = publicKey.toBase58();
    console.log("[solana_wallet] Connected successfully:", pkBase58);
    return pkBase58;
  } catch (e) {
    console.error("[solana_wallet] Connect failed:", e);
    if (e.code) console.error("[solana_wallet] Error code:", e.code);
    if (e.message) console.error("[solana_wallet] Error message:", e.message);
    return JSON.stringify({
      __wallet_error__: true,
      code: e.code || null,
      message: e.message || "Unknown wallet error",
      logs: null,
    });
  }
}

/**
 * Internal: simple promise-based sleep.
 */
function sleep(ms) {
  return new Promise(function (resolve) {
    setTimeout(resolve, ms);
  });
}

/**
 * Get the currently connected wallet's public key (base58) without prompting.
 * Returns null if not connected.
 *
 * @param {string} walletName - Name of the wallet
 * @returns {Promise<string|null>} Base58-encoded public key, or null
 */
export async function getConnectedPublicKey(walletName) {
  try {
    var provider = getProvider(walletName);
    if (!provider) return null;

    // Check if already connected
    if (provider.isConnected && provider.publicKey) {
      return provider.publicKey.toBase58();
    }

    // Try to get public key without prompting (some wallets support this)
    try {
      var response = await provider.connect({ onlyIfTrusted: true });
      return response.publicKey ? response.publicKey.toBase58() : null;
    } catch (_e) {
      return null;
    }
  } catch (_e) {
    return null;
  }
}

/**
 * Sign and send a base64-encoded serialized transaction.
 *
 * Decodes the transaction, signs it with the connected wallet,
 * and sends it to the Solana network.
 *
 * @param {string} walletName - Name of the wallet
 * @param {string} transactionB64 - Base64-encoded serialized transaction
 * @returns {Promise<string|null>} Transaction signature (base58), or null on failure
 */
var web3Promise = null;

function loadWeb3() {
  if (!web3Promise) {
    web3Promise = new Promise(function (resolve, reject) {
      if (window.solanaWeb3) {
        resolve(window.solanaWeb3);
        return;
      }
      var script = document.createElement("script");
      script.src = "https://unpkg.com/@solana/web3.js@1.95.3/lib/index.iife.min.js";
      script.onload = function () {
        if (window.solanaWeb3) {
          console.log("[solana_wallet] Loaded @solana/web3.js from unpkg");
          resolve(window.solanaWeb3);
        } else {
          tryFallback();
        }
      };
      script.onerror = function () {
        tryFallback();
      };
      document.head.appendChild(script);

      function tryFallback() {
        console.log("[solana_wallet] unpkg failed, trying jsdelivr fallback...");
        var fallbackScript = document.createElement("script");
        fallbackScript.src = "https://cdn.jsdelivr.net/npm/@solana/web3.js@1.95.3/lib/index.iife.min.js";
        fallbackScript.onload = function () {
          if (window.solanaWeb3) {
            console.log("[solana_wallet] Loaded @solana/web3.js from jsdelivr");
            resolve(window.solanaWeb3);
          } else {
            reject(new Error("@solana/web3.js loaded from jsdelivr but solanaWeb3 global not found"));
          }
        };
        fallbackScript.onerror = function () {
          reject(new Error("Failed to load @solana/web3.js from both unpkg and jsdelivr"));
        };
        document.head.appendChild(fallbackScript);
      }
    });
  }
  return web3Promise;
}

async function getRpcUrl() {
  try {
    var response = await fetch("/api/health");
    if (response.ok) {
      var data = await response.json();
      var cluster = data.cluster || "devnet";
      if (cluster === "mainnet-beta") {
        return "https://api.mainnet-beta.solana.com";
      } else if (cluster === "testnet") {
        return "https://api.testnet.solana.com";
      } else if (cluster === "localnet") {
        return "http://localhost:8899";
      }
    }
  } catch (e) {
    console.warn("[solana_wallet] Failed to fetch cluster from /api/health:", e);
  }
  return "https://api.devnet.solana.com"; // Fallback to devnet
}

/**
 * Sign and send a base64-encoded serialized transaction.
 *
 * Decodes the transaction, signs it with the connected wallet,
 * and sends it to the Solana network.
 *
 * @param {string} walletName - Name of the wallet
 * @param {string} transactionB64 - Base64-encoded serialized transaction
 * @returns {Promise<string|null>} Transaction signature (base58), or null on failure
 */
export async function signAndSendTransaction(walletName, transactionB64) {
  if (!walletName || !walletName.trim()) {
    console.error("[solana_wallet] signAndSendTransaction: empty wallet name");
    return JSON.stringify({
      __wallet_error__: true,
      code: null,
      message: "No wallet selected",
      logs: null,
    });
  }
  try {
    var provider = getProvider(walletName);
    if (!provider) {
      console.error("[solana_wallet] Wallet not found:", walletName);
      return JSON.stringify({
        __wallet_error__: true,
        code: null,
        message: "Wallet not found. Please connect your wallet first.",
        logs: null,
      });
    }

    // Load web3.js dynamically
    var web3 = await loadWeb3();

    // Decode base64 to Uint8Array
    var binaryString = atob(transactionB64);
    var bytes = new Uint8Array(binaryString.length);
    for (var i = 0; i < binaryString.length; i++) {
      bytes[i] = binaryString.charCodeAt(i);
    }

    // Deserialize transaction bytes into a real @solana/web3.js Transaction/VersionedTransaction
    var tx;
    try {
      // In versioned transactions, the first byte of the serialized message (after signatures) has MSB set.
      var offset = 0;
      var numSigs = 0;
      var shift = 0;
      while (true) {
        var byte = bytes[offset++];
        numSigs |= (byte & 0x7f) << shift;
        if ((byte & 0x80) === 0) break;
        shift += 7;
      }
      offset += numSigs * 64; // skip signatures
      var firstMessageByte = bytes[offset];
      var isVersioned = (firstMessageByte & 0x80) !== 0;

      if (isVersioned && web3.VersionedTransaction) {
        tx = web3.VersionedTransaction.deserialize(bytes);
        console.log("[solana_wallet] Deserialized versioned transaction");
      } else {
        tx = web3.Transaction.from(bytes);
        console.log("[solana_wallet] Deserialized legacy transaction");
      }
    } catch (err) {
      console.warn("[solana_wallet] Versioned/detailed deserialization check failed, falling back to legacy:", err);
      tx = web3.Transaction.from(bytes);
    }

    // Use signTransaction first to allow manual broadcast via the correct RPC connection.
    // This avoids wallet-side network mismatches (e.g., wallet on Mainnet but app on Devnet)
    // since the transaction is broadcast using the dApp's connection rather than the wallet's internally configured node.
    if (provider.signTransaction) {
      console.log("[solana_wallet] Using signTransaction + manual broadcast to avoid network mismatch");
      var signedTx = await provider.signTransaction(tx);
      
      var rpcUrl = await getRpcUrl();
      var connection = new web3.Connection(rpcUrl, "confirmed");
      console.log("[solana_wallet] Sending raw transaction to RPC:", rpcUrl);
      var signature = await connection.sendRawTransaction(signedTx.serialize(), {
        skipPreflight: true,
      });
      console.log("[solana_wallet] Transaction sent successfully, signature:", signature);
      return signature;
    }

    // Fallback: signAndSendTransaction
    if (provider.signAndSendTransaction) {
      console.log("[solana_wallet] signTransaction not available, using signAndSendTransaction fallback");
      var result = await provider.signAndSendTransaction(tx, {
        skipPreflight: true,
      });
      return result.signature || result.toString();
    }

    console.error(
      "[solana_wallet] Wallet does not support signAndSendTransaction or signTransaction",
    );
    return JSON.stringify({
      __wallet_error__: true,
      code: null,
      message: "Wallet does not support transaction signing",
      logs: null,
    });
  } catch (e) {
    console.error("[solana_wallet] Sign and send failed:", e);
    if (e.message) {
      console.error("[solana_wallet] Error message:", e.message);
    }
    // Log wallet-specific error details if available
    if (e.error) {
      console.error("[solana_wallet] Error details:", JSON.stringify(e.error));
    }
    if (e.logs) {
      console.error("[solana_wallet] Program logs:", e.logs);
    }
    if (e.data) {
      console.error("[solana_wallet] Error data:", JSON.stringify(e.data));
    }

    // Extract program error logs from various wallet error formats
    var logs = e.logs || (e.error && e.error.logs) || null;
    var errorCode = e.code || (e.error && e.error.code) || null;

    // Check for instruction error in message
    var msg = e.message || "Unknown transaction error";
    if (e.error && e.error.message) {
      msg = e.error.message;
    }
    if (e.data && e.data.err && e.data.err.InstructionError) {
      msg = "Instruction error: " + JSON.stringify(e.data.err.InstructionError);
    }

    return JSON.stringify({
      __wallet_error__: true,
      code: errorCode,
      message: msg,
      logs: logs,
    });
  }
}

/**
 * Fetch the serialized deposit transaction from the Solana Pay callback URL.
 * The callback URL is the part after "solana:" in the Solana Pay URL.
 *
 * @param {string} callbackUrl - Full HTTPS URL to fetch the transaction from
 * @returns {Promise<string|null>} Base64-encoded transaction, or null on failure
 */
export async function fetchTransactionFromCallback(callbackUrl) {
  try {
    var response = await fetch(callbackUrl, {
      method: "GET",
      headers: {
        Accept: "application/json",
      },
    });

    if (!response.ok) {
      console.error(
        "[solana_wallet] Callback fetch failed:",
        response.status,
        response.statusText,
      );
      return null;
    }

    var data = await response.json();

    // Solana Pay Transaction Request response format: { transaction: string, message: string }
    if (data.transaction) {
      return data.transaction;
    }

    // Also check if wrapped in ApiResponse format: { success: true, data: { transaction: ... } }
    if (data.data && data.data.transaction) {
      return data.data.transaction;
    }

    console.error("[solana_wallet] No transaction in callback response");
    return null;
  } catch (e) {
    console.error("[solana_wallet] Callback fetch error:", e);
    return null;
  }
}

/**
 * Check if a wallet provider is available.
 *
 * @param {string} walletName - Name of the wallet to check
 * @returns {boolean} True if the wallet is detected
 */
export function isWalletAvailable(walletName) {
  return getProvider(walletName) !== null;
}

// ---------------------------------------------------------------------------
// Network / Cluster Detection (SEC-014)
// ---------------------------------------------------------------------------

/**
 * Known Solana genesis hashes for cluster identification.
 *
 * These are the first block hashes on each network and never change.
 * Source: https://docs.solana.com/cluster/rpc-endpoints
 */
var GENESIS_HASHES = {
  // Devnet
  EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG: "devnet",
  // Mainnet Beta
  "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d": "mainnet-beta",
  // Testnet
  "4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY": "testnet",
  // Localnet (default for solana-test-validator / surfnet)
  GH7ome3E1hLUVh7MaLFd2qqL3VJtHBOkPM7TFhXGmWUL: "localnet",
};

/**
 * Detect the wallet's currently connected cluster by reading the
 * genesis hash from the wallet's RPC connection.
 *
 * This is used to prevent signing transactions on the wrong network.
 * For example, if the app expects devnet but the wallet is on mainnet,
 * the user could accidentally spend real SOL.
 *
 * @param {string} walletName - Name of the wallet (e.g. "Phantom")
 * @returns {Promise<string|null>} Cluster name ("devnet", "mainnet-beta", "testnet", "localnet") or null on failure
 */
export async function getWalletCluster(walletName) {
  try {
    var provider = getProvider(walletName);
    if (!provider) {
      console.warn(
        "[solana_wallet] getWalletCluster: provider not found for",
        walletName,
      );
      return null;
    }

    // Most wallet providers expose a .connection or .rpcEndpoint property.
    // We can use this to query the genesis hash via RPC.
    var rpcUrl = null;

    // Phantom and similar wallets expose connection.cluster or connection.rpcEndpoint
    if (provider.connection) {
      if (provider.connection.rpcEndpoint) {
        rpcUrl = provider.connection.rpcEndpoint;
      } else if (provider.connection._rpcEndpoint) {
        rpcUrl = provider.connection._rpcEndpoint;
      }
    }

    // Some wallets expose the cluster directly
    // Phantom: provider._connection or provider.connection.cluster
    if (!rpcUrl && provider.connection && provider.connection.endpoint) {
      rpcUrl = provider.connection.endpoint;
    }

    // If we have an RPC URL, query the genesis hash
    if (rpcUrl) {
      var genesisHash = await fetchGenesisHash(rpcUrl);
      if (genesisHash) {
        var cluster = GENESIS_HASHES[genesisHash];
        if (cluster) {
          console.log(
            "[solana_wallet] Wallet cluster detected:",
            cluster,
            "(genesis:",
            genesisHash.substring(0, 12) + "...)",
          );
          return cluster;
        }
        // Unknown genesis hash — could be a custom local validator
        console.warn(
          "[solana_wallet] Unknown genesis hash:",
          genesisHash,
          "— treating as localnet",
        );
        return "localnet";
      }
    }

    // Fallback: try to read cluster directly from provider
    // Some wallets expose this property
    if (provider.cluster) {
      console.log(
        "[solana_wallet] Cluster from provider.cluster:",
        provider.cluster,
      );
      return provider.cluster;
    }

    // Last resort: guess based on the RPC URL pattern
    if (rpcUrl) {
      if (rpcUrl.indexOf("mainnet") !== -1) return "mainnet-beta";
      if (rpcUrl.indexOf("testnet") !== -1) return "testnet";
      if (
        rpcUrl.indexOf("localhost") !== -1 ||
        rpcUrl.indexOf("127.0.0.1") !== -1
      )
        return "localnet";
      // Default to devnet for unknown URLs
      console.warn(
        "[solana_wallet] Cannot determine cluster from RPC URL, assuming devnet:",
        rpcUrl,
      );
      return "devnet";
    }

    // No connection info available — cannot determine cluster
    console.warn(
      "[solana_wallet] Cannot determine wallet cluster — no RPC endpoint found",
    );
    return null;
  } catch (e) {
    console.error("[solana_wallet] getWalletCluster error:", e);
    return null;
  }
}

/**
 * Fetch the genesis hash from an RPC endpoint.
 * @param {string} rpcUrl - RPC endpoint URL
 * @returns {Promise<string|null>} Genesis hash (base58), or null on failure
 */
async function fetchGenesisHash(rpcUrl) {
  try {
    var response = await fetch(rpcUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "getGenesisHash",
      }),
    });

    if (!response.ok) return null;

    var data = await response.json();
    if (data.result) {
      return data.result;
    }
    return null;
  } catch (e) {
    console.warn("[solana_wallet] fetchGenesisHash failed:", e);
    return null;
  }
}

// ---------------------------------------------------------------------------
// Pre-sign Simulation (Solana Foundation Security Checklist)
// ---------------------------------------------------------------------------

/**
 * Simulate a transaction before signing to verify it will succeed.
 *
 * Follows the Solana Foundation Security Checklist recommendation:
 * "Simulate first: Call simulateTransaction and surface results before
 * requesting a real signature."
 *
 * Uses the wallet's RPC endpoint for simulation. The transaction is simulated
 * with sigVerify=false (unsigned), which still checks program logic, account
 * data, and instruction validity.
 *
 * @param {string} walletName - Name of the wallet (for RPC endpoint discovery)
 * @param {string} transactionB64 - Base64-encoded serialized transaction
 * @returns {Promise<string>} JSON string: { ok: true, logs: [...] } or
 *          { ok: false, error: "...", logs: [...] }
 */
export async function simulateTransactionB64(walletName, transactionB64) {
  try {
    // Find the RPC URL from the wallet provider
    var provider = getProvider(walletName);
    var rpcUrl = null;

    if (provider && provider.connection) {
      rpcUrl =
        provider.connection.rpcEndpoint ||
        provider.connection._rpcEndpoint ||
        provider.connection.endpoint ||
        null;
    }

    if (!rpcUrl) {
      // Cannot simulate without RPC endpoint — return OK (don't block signing)
      console.warn("[solana_wallet] simulate: no RPC URL, skipping simulation");
      return JSON.stringify({ ok: true, skipped: true, logs: [] });
    }

    // Decode base64 transaction
    var binaryString = atob(transactionB64);
    var bytes = new Uint8Array(binaryString.length);
    for (var i = 0; i < binaryString.length; i++) {
      bytes[i] = binaryString.charCodeAt(i);
    }

    // Encode bytes as base58 for the RPC call
    // Solana RPC simulateTransaction expects the transaction as a base58 or base64 string
    var transactionB58 = base58Encode(bytes);

    // Call simulateTransaction RPC method
    var response = await fetch(rpcUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "simulateTransaction",
        params: [
          transactionB58,
          {
            sigVerify: false,
            replaceRecentBlockhash: true,
            commitment: "processed",
          },
        ],
      }),
    });

    if (!response.ok) {
      console.warn("[solana_wallet] simulate: RPC returned", response.status);
      return JSON.stringify({
        ok: true,
        skipped: true,
        logs: [],
      });
    }

    var data = await response.json();

    if (data.error) {
      console.warn("[solana_wallet] simulate: RPC error:", data.error.message);
      return JSON.stringify({
        ok: false,
        error: data.error.message,
        logs: [],
      });
    }

    var simResult = data.value || data.result;
    if (!simResult) {
      // Unexpected response format — don't block
      return JSON.stringify({ ok: true, skipped: true, logs: [] });
    }

    if (simResult.err) {
      var errMsg =
        typeof simResult.err === "string"
          ? simResult.err
          : JSON.stringify(simResult.err);
      console.warn("[solana_wallet] simulate: transaction would fail:", errMsg);
      return JSON.stringify({
        ok: false,
        error: errMsg,
        logs: simResult.logs || [],
      });
    }

    console.log(
      "[solana_wallet] simulate: transaction OK",
      simResult.logs ? simResult.logs.length + " log lines" : "",
    );
    return JSON.stringify({
      ok: true,
      skipped: false,
      logs: simResult.logs || [],
    });
  } catch (e) {
    // Simulation failure should not block signing — log and continue
    console.warn("[solana_wallet] simulate: error (not blocking):", e);
    return JSON.stringify({ ok: true, skipped: true, logs: [] });
  }
}

/**
 * Encode a Uint8Array as base58 (Bitcoin alphabet).
 * Used for RPC calls that expect base58-encoded transactions.
 */
var BASE58_ALPHABET =
  "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function base58Encode(bytes) {
  // Count leading zeros
  var zeros = 0;
  for (var i = 0; i < bytes.length; i++) {
    if (bytes[i] === 0) zeros++;
    else break;
  }

  // Convert to big-endian number
  var num = BigInt(0);
  for (var j = zeros; j < bytes.length; j++) {
    num = num * BigInt(256) + BigInt(bytes[j]);
  }

  // Encode to base58
  var result = "";
  while (num > BigInt(0)) {
    var remainder = num % BigInt(58);
    num = num / BigInt(58);
    result = BASE58_ALPHABET[Number(remainder)] + result;
  }

  // Add leading '1's for leading zeros
  for (var k = 0; k < zeros; k++) {
    result = "1" + result;
  }

  return result || "1";
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/**
 * Get the wallet provider object for a given wallet name.
 */
function getProvider(walletName) {
  var name = walletName.toLowerCase();

  // Phantom: check window.solana first, then window.phantom.solana
  if (name === "phantom") {
    if (window.solana && window.solana.isPhantom) {
      return window.solana;
    }
    if (
      window.phantom &&
      window.phantom.solana &&
      window.phantom.solana.isPhantom
    ) {
      return window.phantom.solana;
    }
    // Generic fallback: any window.solana with connect method
    if (window.solana && window.solana.connect) {
      return window.solana;
    }
  }

  // Backpack: check window.backpack.solana, then legacy _isBackpack
  if (name === "backpack") {
    if (window.backpack && window.backpack.solana) {
      return window.backpack.solana;
    }
    if (window.backpack && window.backpack._isBackpack) {
      return window.backpack;
    }
  }

  // Solflare: check window.solflare
  if (name === "solflare") {
    if (window.solflare && window.solflare.isSolflare) {
      return window.solflare;
    }
    // Fallback: any solflare object with connect
    if (window.solflare && window.solflare.connect) {
      return window.solflare;
    }
  }

  // Coinbase: check window.coinbaseSolana
  if (name === "coinbase" && window.coinbaseSolana) {
    return window.coinbaseSolana;
  }

  // Generic fallback for wallet names not matched above
  // Try window[name] and window[name].solana
  var w = window[name];
  if (w && w.solana && w.solana.connect) {
    return w.solana;
  }
  if (w && w.connect) {
    return w;
  }

  return null;
}
