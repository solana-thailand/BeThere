/**
 * Event Check-In - Admin Dashboard JavaScript
 * Handles stats display, attendee list, QR generation, and recent check-ins.
 * Auth is cookie-based (HTTP-only cookies set by server).
 */

// ===== State =====
let allAttendees = [];
let filteredAttendees = [];
let stats = null;
let isLoading = false;

// ===== Initialization =====

window.addEventListener("DOMContentLoaded", async () => {
  const path = window.location.pathname;
  if (path !== "/admin.html") return;

  // Require authentication (async — checks cookie via API)
  if (!(await requireAuth())) return;

  // Load user info (already cached from requireAuth check)
  loadUserInfo();

  // Set up event listeners
  setupEventListeners();

  // Load initial data
  loadDashboard();
});

// ===== User Info =====

function loadUserInfo() {
  const user = getUser();
  if (user && user.email) {
    const userEl = document.getElementById("userEmail");
    if (userEl) userEl.textContent = user.email;
  }
}

// ===== Event Listeners =====

function setupEventListeners() {
  const refreshBtn = document.getElementById("refreshBtn");
  if (refreshBtn) {
    refreshBtn.addEventListener("click", () => loadDashboard());
  }

  const generateQrBtn = document.getElementById("generateQrBtn");
  if (generateQrBtn) {
    generateQrBtn.addEventListener("click", handleGenerateQrs);
  }

  const searchInput = document.getElementById("searchInput");
  if (searchInput) {
    searchInput.addEventListener(
      "input",
      debounce((e) => {
        filterAttendees(e.target.value);
      }, 300),
    );
  }

  document.querySelectorAll(".tab").forEach((tab) => {
    tab.addEventListener("click", () => {
      switchTab(tab.dataset.tab);
    });
  });
}

// ===== Dashboard =====

async function loadDashboard() {
  if (isLoading) return;
  isLoading = true;
  showLoadingState(true);

  try {
    const data = await api.getAttendees();

    if (!data || !data.success) {
      showToast(data?.error || "Failed to load attendees", "error");
      return;
    }

    allAttendees = data.data.attendees || [];
    filteredAttendees = [...allAttendees];
    stats = data.data.stats || null;

    renderStats();
    renderAttendeeList();
    renderRecentCheckIns();
  } catch (err) {
    console.error("Failed to load dashboard:", err);
    showToast("Failed to load dashboard data", "error");
  } finally {
    isLoading = false;
    showLoadingState(false);
  }
}

// ===== Stats =====

function renderStats() {
  if (!stats) return;

  const totalEl = document.getElementById("statTotal");
  const checkedInEl = document.getElementById("statCheckedIn");
  const remainingEl = document.getElementById("statRemaining");

  if (totalEl) totalEl.textContent = stats.total_approved || 0;
  if (checkedInEl) checkedInEl.textContent = stats.total_checked_in || 0;
  if (remainingEl) remainingEl.textContent = stats.total_remaining || 0;

  const progressFill = document.getElementById("progressFill");
  const progressText = document.getElementById("progressText");

  if (progressFill) {
    const percentage = stats.check_in_percentage || 0;
    progressFill.style.width = `${percentage}%`;
  }

  if (progressText) {
    progressText.textContent = `${Math.round(stats.check_in_percentage || 0)}%`;
  }
}

// ===== Attendee List =====

function filterAttendees(query) {
  const q = (query || "").toLowerCase().trim();

  if (!q) {
    filteredAttendees = [...allAttendees];
  } else {
    filteredAttendees = allAttendees.filter((a) => {
      const name = (a.name || "").toLowerCase();
      const email = (a.email || "").toLowerCase();
      const apiId = (a.api_id || "").toLowerCase();
      const ticket = (a.ticket_name || "").toLowerCase();

      return (
        name.includes(q) ||
        email.includes(q) ||
        apiId.includes(q) ||
        ticket.includes(q)
      );
    });
  }

  renderAttendeeList();
}

function renderAttendeeList() {
  const listEl = document.getElementById("attendeeList");
  const countEl = document.getElementById("attendeeCount");
  const emptyEl = document.getElementById("emptyState");

  if (!listEl) return;

  if (countEl) {
    countEl.textContent = `${filteredAttendees.length} attendee${filteredAttendees.length !== 1 ? "s" : ""}`;
  }

  if (filteredAttendees.length === 0) {
    listEl.innerHTML = "";
    if (emptyEl) emptyEl.classList.remove("hidden");
    return;
  }

  if (emptyEl) emptyEl.classList.add("hidden");

  // Sort: not checked in first, then by name
  const sorted = [...filteredAttendees].sort((a, b) => {
    const aChecked = !!a.checked_in_at;
    const bChecked = !!b.checked_in_at;

    if (aChecked !== bChecked) return aChecked ? 1 : -1;
    return (a.name || "").localeCompare(b.name || "");
  });

  listEl.innerHTML = sorted
    .map((attendee) => {
      const isCheckedIn = !!attendee.checked_in_at;
      const badgeClass = isCheckedIn ? "badge-success" : "badge-warning";
      const badgeText = isCheckedIn ? "Checked In" : "Pending";
      const badgeIcon = isCheckedIn ? "&#10003;" : "&#9203;";

      return `
        <div class="attendee-item" data-id="${escapeAttr(attendee.api_id)}">
          <div class="attendee-info">
            <div class="attendee-name">${escapeHtml(attendee.name || "Unknown")}</div>
            <div class="attendee-email">${escapeHtml(attendee.email || "")}</div>
            ${attendee.ticket_name ? `<div style="font-size:0.75rem;color:var(--text-muted);margin-top:2px">${escapeHtml(attendee.ticket_name)}</div>` : ""}
          </div>
          <div class="attendee-status">
            <span class="badge ${badgeClass}">
              ${badgeIcon} ${badgeText}
            </span>
            ${isCheckedIn ? `<div style="font-size:0.7rem;color:var(--text-muted);margin-top:4px;text-align:right">${timeAgo(attendee.checked_in_at)}</div>` : ""}
          </div>
        </div>
      `;
    })
    .join("");
}

// ===== Recent Check-Ins =====

function renderRecentCheckIns() {
  const listEl = document.getElementById("recentCheckIns");

  if (!listEl || !stats) return;

  const recent = stats.recent_check_ins || [];

  if (recent.length === 0) {
    listEl.innerHTML =
      '<p style="color:var(--text-muted);text-align:center;padding:1rem">No check-ins yet</p>';
    return;
  }

  listEl.innerHTML = recent
    .map((checkIn) => {
      return `
        <div class="attendee-item">
          <div class="attendee-info">
            <div class="attendee-name">${escapeHtml(checkIn.name || "Unknown")}</div>
            <div class="attendee-email" style="font-size:0.8rem">${escapeHtml(checkIn.api_id || "")}</div>
          </div>
          <div class="attendee-status text-right">
            <div style="font-size:0.8rem;color:var(--text-secondary)">${formatTimestamp(checkIn.checked_in_at)}</div>
          </div>
        </div>
      `;
    })
    .join("");
}

// ===== QR Code Generation =====

async function handleGenerateQrs() {
  const btn = document.getElementById("generateQrBtn");
  const resultEl = document.getElementById("qrResult");

  if (btn) {
    btn.disabled = true;
    btn.innerHTML = '<span class="spinner"></span> Generating...';
  }

  if (resultEl) resultEl.classList.add("hidden");

  try {
    const data = await api.generateQrs();

    if (!data || !data.success) {
      showToast(data?.error || "Failed to generate QR codes", "error");
      return;
    }

    const result = data.data;
    showToast(
      `Generated ${result.generated} QR codes (${result.skipped} skipped)`,
      "success",
    );

    // Show result details
    if (resultEl) {
      let html = '<div class="qr-result">';
      html += `<p><strong>Total:</strong> ${result.total}</p>`;
      html += `<p><strong>Generated:</strong> ${result.generated}</p>`;
      html += `<p><strong>Skipped:</strong> ${result.skipped}</p>`;

      const generated = (result.details || []).filter(
        (d) => d.status === "generated",
      );
      if (generated.length > 0) {
        html +=
          '<div style="margin-top:1rem;max-height:200px;overflow-y:auto">';
        html += generated
          .map(
            (d) => `
          <div class="attendee-item" style="padding:0.5rem">
            <div class="attendee-info">
              <div class="attendee-name" style="font-size:0.85rem">${escapeHtml(d.name)}</div>
            </div>
            <span class="badge badge-success" style="font-size:0.7rem">Generated</span>
          </div>
        `,
          )
          .join("");
        html += "</div>";
      }

      html += "</div>";
      resultEl.innerHTML = html;
      resultEl.classList.remove("hidden");
    }

    // Refresh attendee list
    await loadDashboard();
  } catch (err) {
    console.error("QR generation error:", err);
    showToast("Failed to generate QR codes", "error");
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.innerHTML = "Generate QR Codes";
    }
  }
}

// ===== Tab Navigation =====

function switchTab(tabName) {
  document.querySelectorAll(".tab").forEach((tab) => {
    tab.classList.toggle("active", tab.dataset.tab === tabName);
  });

  document.querySelectorAll(".tab-panel").forEach((panel) => {
    panel.classList.toggle("hidden", panel.id !== `panel-${tabName}`);
  });
}

// ===== UI Helpers =====

function showLoadingState(show) {
  const loadingEl = document.getElementById("dashboardLoading");
  const contentEl = document.getElementById("dashboardContent");

  if (loadingEl) loadingEl.classList.toggle("hidden", !show);
  if (contentEl) contentEl.classList.toggle("hidden", show);
}

// ===== Utility =====

function escapeHtml(text) {
  if (!text) return "";
  const div = document.createElement("div");
  div.appendChild(document.createTextNode(text));
  return div.innerHTML;
}

function escapeAttr(text) {
  if (!text) return "";
  return text
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}
