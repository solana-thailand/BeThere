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
let lastQrResult = null;

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
  } catch (err) {
    console.error("[dashboard] API error:", err);
    showToast("Failed to load dashboard data", "error");
    return;
  } finally {
    isLoading = false;
    showLoadingState(false);
  }

  // Render separately so UI errors don't mask the actual API error
  try {
    renderStats();
    renderAttendeeList();
    renderRecentCheckIns();
  } catch (err) {
    console.error("[dashboard] Render error:", err);
    showToast(
      "Dashboard loaded but rendering failed. Check browser console (F12).",
      "warning",
    );
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

  // Compute participation type counts from attendees
  const inPersonCount = allAttendees.filter((a) => {
    const p = (a.participation_type || "").toLowerCase();
    return p.includes("in-person") || p.includes("in person");
  }).length;
  const onlineCount = allAttendees.length - inPersonCount;

  // Add or update participation stat cards
  const statsGrid = document.querySelector(".stats-grid");
  if (statsGrid) {
    let inPersonCard = document.getElementById("statInPerson");
    let onlineCard = document.getElementById("statOnline");

    if (!inPersonCard) {
      inPersonCard = document.createElement("div");
      inPersonCard.className = "stat-card info";
      inPersonCard.id = "statInPerson";
      inPersonCard.innerHTML =
        '<div class="stat-value">0</div><div class="stat-label">In-Person</div>';
      statsGrid.appendChild(inPersonCard);
    }
    if (!onlineCard) {
      onlineCard = document.createElement("div");
      onlineCard.className = "stat-card warning";
      onlineCard.id = "statOnline";
      onlineCard.innerHTML =
        '<div class="stat-value">0</div><div class="stat-label">Online</div>';
      statsGrid.appendChild(onlineCard);
    }

    inPersonCard.querySelector(".stat-value").textContent = inPersonCount;
    onlineCard.querySelector(".stat-value").textContent = onlineCount;
  }

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
      const participation = getParticipationBadge(attendee.participation_type);

      return `
        <div class="attendee-item" data-id="${escapeAttr(attendee.api_id)}">
          <div class="attendee-info">
            <div class="attendee-name">${escapeHtml(attendee.name || "Unknown")}</div>
            <div class="attendee-email">${escapeHtml(attendee.email || "")}</div>
            ${attendee.ticket_name ? `<div style="font-size:0.75rem;color:var(--text-muted);margin-top:2px">${escapeHtml(attendee.ticket_name)}</div>` : ""}
          </div>
          <div class="attendee-status">
            <span class="badge ${participation.class}" style="margin-bottom:4px">${participation.label}</span>
            <span class="badge ${badgeClass}">
              ${badgeIcon} ${badgeText}
            </span>
            ${isCheckedIn ? `<div style="font-size:0.7rem;color:var(--text-muted);margin-top:4px;text-align:right">${timeAgo(attendee.checked_in_at)}${attendee.checked_in_by ? ` by ${escapeHtml(attendee.checked_in_by)}` : ""}</div>` : ""}
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
      // Look up participation_type from allAttendees by api_id
      const matchedAttendee = allAttendees.find(
        (a) => a.api_id === checkIn.api_id,
      );
      const participation = getParticipationBadge(
        matchedAttendee?.participation_type,
      );

      return `
        <div class="attendee-item">
          <div class="attendee-info">
            <div class="attendee-name">${escapeHtml(checkIn.name || "Unknown")}</div>
            <div class="attendee-email" style="font-size:0.8rem">${escapeHtml(checkIn.api_id || "")}</div>
          </div>
          <div class="attendee-status text-right">
            <span class="badge ${participation.class}" style="font-size:0.7rem;margin-bottom:4px">${participation.label}</span>
            <div style="font-size:0.8rem;color:var(--text-secondary)">${formatTimestamp(checkIn.checked_in_at)}${checkIn.checked_in_by ? ` by ${escapeHtml(checkIn.checked_in_by)}` : ""}</div>
          </div>
        </div>
      `;
    })
    .join("");
}

// ===== QR Code Generation =====

/**
 * QR Code generation handler.
 * @param {boolean} force - If true, regenerate QR URLs even for attendees with existing ones.
 */
async function handleGenerateQrs(force = false) {
  const btn = document.getElementById("generateQrBtn");
  const resultEl = document.getElementById("qrResult");

  if (btn) {
    btn.disabled = true;
    btn.innerHTML = '<span class="spinner"></span> Generating...';
  }

  if (resultEl) resultEl.classList.add("hidden");

  try {
    const data = await api.generateQrs(force);

    if (!data || !data.success) {
      showToast(data?.error || "Failed to generate QR codes", "error");
      return;
    }

    const result = data.data;
    lastQrResult = result;

    if (result.generated > 0) {
      showToast(
        `Generated ${result.generated} QR codes (${result.skipped} skipped)`,
        "success",
      );
    } else {
      showToast(
        `All ${result.skipped} approved attendees already have QR codes. Use "Force Regenerate" to overwrite.`,
        "warning",
      );
    }

    // Show result details
    if (resultEl) {
      let html = '<div class="qr-result">';

      // Summary row
      html +=
        '<div style="display:flex;gap:1.5rem;margin-bottom:0.75rem;flex-wrap:wrap">';
      html += `<div><strong>Total Approved:</strong> ${result.total}</div>`;
      html += `<div style="color:var(--success)"><strong>Generated:</strong> ${result.generated}</div>`;
      html += `<div style="color:var(--warning)"><strong>Skipped:</strong> ${result.skipped}</div>`;
      html += "</div>";

      // Force regenerate button (always show after first generation)
      html +=
        '<button class="btn btn-outline btn-sm" onclick="handleGenerateQrs(true)" style="margin-bottom:1rem">';
      html += "&#128260; Force Regenerate All";
      html += "</button>";
      html +=
        '<span style="font-size:0.75rem;color:var(--text-muted);margin-left:0.5rem">Overwrites existing QR URLs</span>';

      const details = result.details || [];

      // Show generated attendees
      const generated = details.filter((d) => d.status === "generated");
      if (generated.length > 0) {
        html +=
          '<div style="margin-top:0.5rem;max-height:200px;overflow-y:auto">';
        html += generated
          .map(
            (d) => `
          <div class="attendee-item" style="padding:0.5rem">
            <div class="attendee-info">
              <div class="attendee-name" style="font-size:0.85rem">${escapeHtml(d.name)}</div>
              <div style="font-size:0.7rem;color:var(--text-muted);word-break:break-all">${escapeHtml(d.qr_code_url || "")}</div>
            </div>
            <span class="badge badge-success" style="font-size:0.7rem">Generated</span>
          </div>
        `,
          )
          .join("");
        html += "</div>";
      }

      // Show skipped attendees
      const skipped = details.filter((d) => d.status === "skipped");
      if (skipped.length > 0 && skipped.length <= 20) {
        html +=
          '<div style="margin-top:0.75rem;font-size:0.8rem;color:var(--text-muted)">Skipped (already have QR URLs):</div>';
        html += '<div style="max-height:150px;overflow-y:auto">';
        html += skipped
          .map(
            (d) => `
          <div class="attendee-item" style="padding:0.4rem;opacity:0.7">
            <div class="attendee-info">
              <div class="attendee-name" style="font-size:0.8rem">${escapeHtml(d.name)}</div>
            </div>
            <span class="badge badge-warning" style="font-size:0.65rem">Skipped</span>
          </div>
        `,
          )
          .join("");
        html += "</div>";
      } else if (skipped.length > 20) {
        html += `<div style="font-size:0.8rem;color:var(--text-muted);margin-top:0.5rem">${skipped.length} attendees skipped (already have QR URLs)</div>`;
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
      btn.innerHTML = "&#127915; Generate QR Codes";
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

// ===== Participation Type Helper =====

function getParticipationBadge(type) {
  if (!type) return { label: "Unknown", class: "badge-warning" };
  const lower = type.toLowerCase();
  if (lower.includes("in-person") || lower.includes("in person")) {
    return { label: "In-Person", class: "badge-info" };
  }
  if (lower.includes("online") || lower.includes("virtual")) {
    return { label: "Online", class: "badge-warning" };
  }
  return {
    label: type.split(":")[0].split("/")[0].trim(),
    class: "badge-warning",
  };
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
