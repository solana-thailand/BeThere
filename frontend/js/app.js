/**
 * Event Check-In - Main Application JavaScript
 * Handles authentication via HTTP-only cookies, API communication, and login page logic.
 */

const API_BASE = window.location.origin + "/api";

// ===== Auth State =====

let currentUser = null;

function getUser() {
  return currentUser;
}

function setUser(user) {
  currentUser = user;
}

function isAuthenticated() {
  return currentUser !== null;
}

/**
 * Check auth by calling /api/auth/me (cookie is sent automatically).
 * Returns true if authenticated, false otherwise.
 */
async function checkAuth() {
  try {
    const response = await fetch(`${API_BASE}/auth/me`, {
      credentials: "include",
      headers: { "Content-Type": "application/json" },
    });

    if (response.ok) {
      const data = await response.json();
      if (data && data.email) {
        currentUser = data;
        return true;
      }
    }

    currentUser = null;
    return false;
  } catch (err) {
    console.error("[auth] checkAuth failed:", err);
    currentUser = null;
    return false;
  }
}

/**
 * Redirect to login page if not authenticated.
 * Checks auth via API call (cookie-based).
 */
async function requireAuth() {
  const ok = await checkAuth();
  if (!ok) {
    window.location.href = "/";
    return false;
  }
  return true;
}

/**
 * Logout: redirect to server logout endpoint which clears cookie.
 */
function logout() {
  window.location.href = "/api/auth/logout";
}

// ===== API Wrapper =====

/**
 * Make an authenticated API request.
 * Cookies are sent automatically via credentials: 'include'.
 * Handles 401 responses by redirecting to login.
 */
async function apiRequest(method, path, body = null) {
  console.log(`[api] ${method} ${path}`, body || "");

  const headers = {
    "Content-Type": "application/json",
  };

  const options = {
    method,
    headers,
    credentials: "include",
  };

  if (body && method !== "GET") {
    options.body = JSON.stringify(body);
  }

  try {
    const response = await fetch(`${API_BASE}${path}`, options);
    console.log(`[api] ${method} ${path} → status:`, response.status);

    if (response.status === 401) {
      console.warn("[api] 401 unauthorized, redirecting to login");
      currentUser = null;
      window.location.href = "/?error=session_expired";
      return null;
    }

    if (response.status === 403) {
      const data = await response.json();
      console.warn("[api] 403 forbidden:", data);
      throw new Error(data.error || "Access denied");
    }

    const json = await response.json();
    console.log(
      `[api] ${method} ${path} response:`,
      JSON.stringify(json).slice(0, 500),
    );
    return json;
  } catch (err) {
    if (err.message === "Access denied") {
      throw err;
    }
    console.error(`[api] ${method} ${path} failed:`, err);
    throw err;
  }
}

/**
 * Convenience methods for API calls.
 */
const api = {
  get: (path) => apiRequest("GET", path),
  post: (path, body) => apiRequest("POST", path, body),
  put: (path, body) => apiRequest("PUT", path, body),
  delete: (path) => apiRequest("DELETE", path),

  // Auth endpoints
  getAuthUrl: () => api.get("/auth/url"),
  getMe: () => api.get("/auth/me"),

  // Attendee endpoints
  getAttendees: () => api.get("/attendees"),
  getAttendee: (id) => api.get(`/attendee/${encodeURIComponent(id)}`),

  // Check-in endpoint
  checkIn: (id) => api.post(`/checkin/${encodeURIComponent(id)}`),

  // QR generation endpoint
  generateQrs: (force = false) =>
    api.post(`/generate-qrs${force ? "?force=true" : ""}`),
};

// ===== Login Page Logic =====

/**
 * Initialize the login page.
 * Handles error display and auto-redirect if already authenticated.
 */
async function initLoginPage() {
  // Handle error from OAuth callback redirect
  const params = new URLSearchParams(window.location.search);
  const error = params.get("error");
  const message = params.get("message");

  if (error) {
    showLoginError(error, message);
    window.history.replaceState({}, "", "/");
  }

  // If already authenticated, redirect to staff page
  const authed = await checkAuth();
  if (authed) {
    window.location.href = "/staff.html";
  }
}

/**
 * Show login error message based on error code.
 */
function showLoginError(error, message) {
  const errorEl = document.getElementById("errorMsg");
  if (!errorEl) return;

  let text = "";

  switch (error) {
    case "not_authorized":
      text =
        "⛔ Access Denied — This system is for authorized staff only. If you are an event attendee, you do not need to log in here. If you believe you should have access, please contact the event organizer.";
      break;
    case "auth_failed":
      text = "Authentication failed. Please try again.";
      if (message) text += " (" + message + ")";
      break;
    case "oauth_failed":
      text = "Google authentication was cancelled or failed.";
      if (message) text += " (" + message + ")";
      break;
    case "missing_code":
      text = "Invalid authentication response. Please try again.";
      break;
    case "token_failed":
      text = "Failed to create session. Please try again.";
      break;
    case "session_expired":
      text = "Your session has expired. Please sign in again.";
      break;
    default:
      text = "An unexpected error occurred. Please try again.";
  }

  errorEl.textContent = text;
  errorEl.classList.add("visible");
  if (error === "not_authorized") {
    errorEl.classList.add("not-authorized");
  }
}

/**
 * Login button click handler.
 * Fetches the Google OAuth URL and redirects the user.
 */
async function login() {
  const btn = document.getElementById("loginBtn");
  const loading = document.getElementById("loading");
  const errorEl = document.getElementById("errorMsg");

  if (btn) {
    btn.disabled = true;
    btn.style.opacity = "0.6";
  }
  if (loading) loading.classList.add("visible");
  if (errorEl) errorEl.classList.remove("visible");

  try {
    const data = await api.getAuthUrl();

    if (data && data.auth_url) {
      window.location.href = data.auth_url;
    } else {
      throw new Error("Failed to get authentication URL");
    }
  } catch (err) {
    console.error("Login error:", err);
    if (errorEl) {
      errorEl.textContent =
        "Failed to connect to the server. Please try again.";
      errorEl.classList.add("visible");
    }
    if (btn) {
      btn.disabled = false;
      btn.style.opacity = "1";
    }
    if (loading) loading.classList.remove("visible");
  }
}

// ===== Utility Functions =====

/**
 * Format an ISO timestamp to a readable string.
 */
function formatTimestamp(isoString) {
  if (!isoString) return "N/A";
  try {
    const date = new Date(isoString);
    return date.toLocaleString("en-US", {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  } catch {
    return isoString;
  }
}

/**
 * Format a relative time string (e.g., "5 minutes ago").
 */
function timeAgo(isoString) {
  if (!isoString) return "";
  try {
    const date = new Date(isoString);
    const now = new Date();
    const seconds = Math.floor((now - date) / 1000);

    if (seconds < 60) return "just now";
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
    return `${Math.floor(seconds / 86400)}d ago`;
  } catch {
    return "";
  }
}

/**
 * Debounce function for search inputs.
 */
function debounce(func, wait) {
  let timeout;
  return function executedFunction(...args) {
    const later = () => {
      clearTimeout(timeout);
      func(...args);
    };
    clearTimeout(timeout);
    timeout = setTimeout(later, wait);
  };
}

/**
 * Show a toast notification.
 */
function showToast(message, type = "info") {
  document.querySelectorAll(".toast").forEach((t) => t.remove());

  const toast = document.createElement("div");
  toast.className = `toast toast-${type}`;

  const colors = {
    success: {
      bg: "rgba(34, 197, 94, 0.15)",
      border: "rgba(34, 197, 94, 0.4)",
      text: "#22c55e",
    },
    error: {
      bg: "rgba(239, 68, 68, 0.15)",
      border: "rgba(239, 68, 68, 0.4)",
      text: "#ef4444",
    },
    warning: {
      bg: "rgba(245, 158, 11, 0.15)",
      border: "rgba(245, 158, 11, 0.4)",
      text: "#f59e0b",
    },
    info: {
      bg: "rgba(59, 130, 246, 0.15)",
      border: "rgba(59, 130, 246, 0.4)",
      text: "#3b82f6",
    },
  };

  const c = colors[type] || colors.info;

  Object.assign(toast.style, {
    position: "fixed",
    top: "1rem",
    right: "1rem",
    padding: "0.85rem 1.25rem",
    background: c.bg,
    border: `1px solid ${c.border}`,
    borderRadius: "8px",
    color: c.text,
    fontSize: "0.9rem",
    fontWeight: "500",
    zIndex: "9999",
    maxWidth: "360px",
    animation: "fadeIn 0.3s ease",
  });

  toast.textContent = message;
  document.body.appendChild(toast);

  setTimeout(() => {
    toast.style.animation = "fadeOut 0.3s ease";
    setTimeout(() => toast.remove(), 300);
  }, 4000);
}

// Add toast animations
const toastStyles = document.createElement("style");
toastStyles.textContent = `
  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(-10px); }
    to { opacity: 1; transform: translateY(0); }
  }
  @keyframes fadeOut {
    from { opacity: 1; transform: translateY(0); }
    to { opacity: 0; transform: translateY(-10px); }
  }
`;
document.head.appendChild(toastStyles);

// ===== Page Initialization =====

window.addEventListener("DOMContentLoaded", () => {
  const path = window.location.pathname;

  // Login page
  if (path === "/" || path === "/index.html") {
    initLoginPage();
  }
});
