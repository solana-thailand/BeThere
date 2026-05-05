/**
 * Solana wallet adapter interop for BeThere deposit flow.
 *
 * Uses the Wallet Standard API to detect, connect, and transact with
 * Solana wallets (Phantom, Backpack, Solflare, etc.).
 *
 * Imported via `#[wasm_bindgen(module = "/js/solana_wallet.js")]` in Rust.
 */

/**
 * Get a list of detected Solana wallet adapter names.
 * Checks window.solana (Phantom), window.backpack (Backpack), window.solflare (Solflare),
 * and the Wallet Standard registry if available.
 *
 * @returns {Array<string>} Array of wallet names, e.g. ["Phantom", "Backpack"]
 */
export function getDetectedWallets() {
  var wallets = [];

  // Legacy adapter detection (most common wallets)
  if (window.solana && window.solana.isPhantom) {
    wallets.push("Phantom");
  }
  if (window.backpack && window.backpack._isBackpack) {
    wallets.push("Backpack");
  }
  if (window.solflare && window.solflare.isSolflare) {
    wallets.push("Solflare");
  }
  if (window.coinbaseSolana) {
    wallets.push("Coinbase");
  }

  // Wallet Standard detection (newer approach)
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

  return wallets;
}

/**
 * Connect to a Solana wallet and return the public key (base58).
 *
 * @param {string} walletName - Name of the wallet to connect (e.g. "Phantom", "Backpack")
 * @returns {Promise<string|null>} Base58-encoded public key, or null on failure
 */
export async function connectWallet(walletName) {
  try {
    var provider = getProvider(walletName);
    if (!provider) {
      console.error("[solana_wallet] Wallet not found:", walletName);
      return null;
    }

    var response = await provider.connect();
    var publicKey = response.publicKey;
    if (!publicKey) {
      console.error("[solana_wallet] No public key in response");
      return null;
    }

    return publicKey.toBase58();
  } catch (e) {
    console.error("[solana_wallet] Connect failed:", e); // eslint-disable-line no-unused-vars
    return null;
  }
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

    // Sign and send the transaction
    // Most wallets support signAndSendTransaction which handles both signing
    // and broadcasting in one call
    if (provider.signAndSendTransaction) {
      var result = await provider.signAndSendTransaction(bytes, {
        skipPreflight: false,
        preflightCommitment: "confirmed",
      });
      return result.signature || result.toString();
    }

    // Fallback: sign separately, then send manually via RPC
    if (provider.signTransaction) {
      var signedTx = await provider.signTransaction(bytes);
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
    console.error("[solana_wallet] Sign and send failed:", e); // eslint-disable-line no-unused-vars
    if (e.message) {
      console.error("[solana_wallet] Error message:", e.message);
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

  // Direct window properties for common wallets
  if (name === "phantom" && window.solana && window.solana.isPhantom) {
    return window.solana;
  }
  if (name === "backpack" && window.backpack && window.backpack._isBackpack) {
    return window.backpack.solana;
  }
  if (name === "solflare" && window.solflare && window.solflare.isSolflare) {
    return window.solflare;
  }
  if (name === "coinbase" && window.coinbaseSolana) {
    return window.coinbaseSolana;
  }

  // Generic fallback: check window.solana (some wallets inject here)
  if (name === "phantom" && window.solana) {
    return window.solana;
  }

  return null;
}
