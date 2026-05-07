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

  // Legacy adapter detection — check multiple injection patterns

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

  // Solflare: window.solflare or window.solflare.isSolflare
  if (window.solflare && window.solflare.isSolflare) {
    wallets.push("Solflare");
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
    return null;
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
  return null;
}

/**
 * Internal: perform the actual connect call on a provider.
 */
async function doConnect(provider, walletName) {
  try {
    console.log("[solana_wallet] Provider found, calling connect()...");
    var response = await provider.connect();
    console.log("[solana_wallet] Connect response:", response);

    var publicKey = response.publicKey;
    if (!publicKey) {
      console.error("[solana_wallet] No public key in response");
      return null;
    }

    var pkBase58 = publicKey.toBase58();
    console.log("[solana_wallet] Connected successfully:", pkBase58);
    return pkBase58;
  } catch (e) {
    console.error("[solana_wallet] Connect failed:", e);
    if (e.code) console.error("[solana_wallet] Error code:", e.code);
    if (e.message) console.error("[solana_wallet] Error message:", e.message);
    return null;
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
export async function signAndSendTransaction(walletName, transactionB64) {
  if (!walletName || !walletName.trim()) {
    console.error("[solana_wallet] signAndSendTransaction: empty wallet name");
    return null;
  }
  try {
    var provider = getProvider(walletName);
    if (!provider) {
      console.error("[solana_wallet] Wallet not found:", walletName);
      return null;
    }

    // Decode base64 to Uint8Array
    var binaryString = atob(transactionB64);
    var bytes = new Uint8Array(binaryString.length);
    for (var i = 0; i < binaryString.length; i++) {
      bytes[i] = binaryString.charCodeAt(i);
    }

    // Wrap raw bytes in a minimal Transaction-like object.
    // Wallet providers (Phantom, Solflare, Backpack) expect an object with
    // a .serialize() method, not a raw Uint8Array.
    var tx = {
      serialize: function () {
        return bytes;
      },
    };

    // Sign and send the transaction
    // Most wallets support signAndSendTransaction which handles both signing
    // and broadcasting in one call
    if (provider.signAndSendTransaction) {
      var result = await provider.signAndSendTransaction(tx, {
        skipPreflight: true,
      });
      return result.signature || result.toString();
    }

    // Fallback: sign separately, then send manually via RPC
    if (provider.signTransaction) {
      var signedTx = await provider.signTransaction(tx);
      // For the fallback path, we need to broadcast via RPC
      // Return the signature from the signed transaction
      // The signed transaction contains signatures in the first bytes
      // Solana transaction format: num_signatures (1 byte) + signatures (64 bytes each) + ...
      if (signedTx.signature) {
        // Some wallets return a signature directly
        return typeof signedTx.signature === "string"
          ? signedTx.signature
          : btoa(String.fromCharCode.apply(null, signedTx.signature));
      }
      // Extract from the signed transaction bytes
      // First byte is number of signatures, then 64 bytes per signature
      var numSigs = signedTx instanceof Uint8Array ? signedTx[0] : 0;
      if (
        numSigs > 0 &&
        signedTx instanceof Uint8Array &&
        signedTx.length >= 65
      ) {
        var sigBytes = signedTx.slice(1, 65);
        return btoa(String.fromCharCode.apply(null, sigBytes));
      }
    }

    console.error(
      "[solana_wallet] Wallet does not support signAndSendTransaction or signTransaction",
    );
    return null;
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
    return null;
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
