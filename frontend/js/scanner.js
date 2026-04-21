/**
 * Event Check-In - QR Scanner Page JavaScript
 * Handles camera-based QR scanning, manual ID input, and check-in flow.
 * Supports two modes:
 *   - Preview: Show attendee info first, then confirm check-in
 *   - Instant: Check in immediately after scan/lookup
 * Auth is cookie-based (HTTP-only cookies set by server).
 */

// ===== State =====
let scanner = null;
let isScanning = false;
let isCheckingIn = false;
let lastScannedId = null;
let scanTimeout = null;

// Preview state: holds attendee data until confirmed
let pendingAttendeeId = null;
let pendingAttendeeData = null;

// ===== Initialization =====

window.addEventListener("DOMContentLoaded", async () => {
  const path = window.location.pathname;
  if (path !== "/staff.html") return;

  // Require authentication (async — checks cookie via API)
  if (!(await requireAuth())) return;

  // Load user info (already cached from requireAuth check)
  loadUserInfo();

  // Check for scan param from QR code URL
  const params = new URLSearchParams(window.location.search);
  const scanId = params.get("scan");
  if (scanId) {
    const url = new URL(window.location);
    url.searchParams.delete("scan");
    window.history.replaceState({}, "", url.pathname);
    processScanResult(scanId);
  }

  // Initialize scanner
  initScanner();

  // Set up event listeners
  setupEventListeners();
});

// ===== User Info =====

function loadUserInfo() {
  const user = getUser();
  if (user && user.email) {
    const userEl = document.getElementById("userEmail");
    if (userEl) userEl.textContent = user.email;
  }
}

// ===== Scanner Setup =====

function initScanner() {
  const scannerArea = document.getElementById("scannerArea");
  if (!scannerArea) return;

  if (typeof Html5Qrcode === "undefined") {
    console.warn("html5-qrcode library not loaded, using manual input only");
    showManualOnly();
    return;
  }

  scanner = new Html5Qrcode("scannerVideo");
}

function setupEventListeners() {
  const startBtn = document.getElementById("startScanBtn");
  if (startBtn) startBtn.addEventListener("click", startScanning);

  const stopBtn = document.getElementById("stopScanBtn");
  if (stopBtn) stopBtn.addEventListener("click", stopScanning);

  const manualForm = document.getElementById("manualForm");
  if (manualForm) manualForm.addEventListener("submit", handleManualCheckIn);

  const newScanBtn = document.getElementById("newScanBtn");
  if (newScanBtn) newScanBtn.addEventListener("click", resetScanner);
}

// ===== Scanning =====

async function startScanning() {
  if (isScanning || !scanner) return;

  const startBtn = document.getElementById("startScanBtn");
  const stopBtn = document.getElementById("stopScanBtn");
  const statusEl = document.getElementById("scannerStatus");

  try {
    if (startBtn) startBtn.classList.add("hidden");
    if (stopBtn) stopBtn.classList.remove("hidden");
    if (statusEl) {
      statusEl.textContent = "Starting camera...";
      statusEl.classList.remove("hidden");
    }

    await scanner.start(
      { facingMode: "environment" },
      {
        fps: 10,
        qrbox: { width: 250, height: 250 },
        aspectRatio: 1.0,
      },
      onScanSuccess,
      onScanFailure,
    );

    isScanning = true;
    if (statusEl) statusEl.textContent = "Scanning... Point camera at QR code";
  } catch (err) {
    console.error("Failed to start scanner:", err);

    if (statusEl) {
      statusEl.textContent =
        "Camera access denied or unavailable. Use manual input below.";
    }
    if (startBtn) startBtn.classList.remove("hidden");
    if (stopBtn) stopBtn.classList.add("hidden");

    showToast("Camera not available. Use manual input.", "warning");
  }
}

function stopScanning() {
  if (!isScanning || !scanner) return;

  scanner
    .stop()
    .then(() => {
      isScanning = false;
      const startBtn = document.getElementById("startScanBtn");
      const stopBtn = document.getElementById("stopScanBtn");
      const statusEl = document.getElementById("scannerStatus");

      if (startBtn) startBtn.classList.remove("hidden");
      if (stopBtn) stopBtn.classList.add("hidden");
      if (statusEl) statusEl.classList.add("hidden");
    })
    .catch((err) => {
      console.error("Failed to stop scanner:", err);
    });
}

function onScanSuccess(decodedText) {
  if (isCheckingIn) return;
  if (decodedText === lastScannedId) {
    if (scanTimeout) clearTimeout(scanTimeout);
    scanTimeout = setTimeout(() => {
      lastScannedId = null;
    }, 3000);
    return;
  }

  lastScannedId = decodedText;
  stopScanning();

  const attendeeId = extractAttendeeId(decodedText);
  if (attendeeId) {
    processScanResult(attendeeId);
  } else {
    showResult(
      "error",
      "Invalid QR Code",
      "The scanned QR code does not contain a valid attendee ID.",
      { rawValue: decodedText },
    );
  }
}

function onScanFailure(_error) {
  // Silent — QR library logs frequent failures during scanning
}

/**
 * Extract attendee ID from scanned QR content.
 * Supports: full URL with ?scan= or ?id=, direct gst- prefix, or raw ID.
 */
function extractAttendeeId(text) {
  if (!text) return null;

  const trimmed = text.trim();

  try {
    const url = new URL(trimmed);
    const scanParam = url.searchParams.get("scan");
    if (scanParam) return scanParam;
    const idParam = url.searchParams.get("id");
    if (idParam) return idParam;
  } catch {
    // Not a URL
  }

  if (trimmed.startsWith("gst-")) return trimmed;
  if (trimmed.length > 0) return trimmed;

  return null;
}

// ===== Manual Check-In =====

async function handleManualCheckIn(event) {
  event.preventDefault();
  console.log("[manual] form submitted");

  const input = document.getElementById("manualIdInput");
  if (!input) {
    console.error("[manual] input element not found");
    return;
  }

  const value = input.value.trim();
  console.log("[manual] input value:", value);
  if (!value) {
    showToast("Please enter an attendee ID", "warning");
    return;
  }

  const attendeeId = extractAttendeeId(value);
  console.log("[manual] extracted attendee ID:", attendeeId);
  if (attendeeId) {
    processScanResult(attendeeId);
  } else {
    showToast("Invalid attendee ID format", "error");
  }
}

// ===== Check-In Flow =====

/**
 * Get the current check-in mode from the page.
 * Returns "preview" or "instant".
 */
function getCheckInMode() {
  if (typeof checkInMode !== "undefined") {
    return checkInMode;
  }
  return "preview";
}

/**
 * Process a scan result — either show preview or check in instantly.
 */
async function processScanResult(attendeeId) {
  if (isCheckingIn) {
    console.warn("[checkin] already checking in, skipping");
    return;
  }
  isCheckingIn = true;

  console.log(
    "[checkin] processScanResult called with:",
    attendeeId,
    "mode:",
    getCheckInMode(),
  );
  showLoading("Looking up attendee...");

  try {
    const data = await api.getAttendee(attendeeId);
    console.log("[checkin] getAttendee response:", JSON.stringify(data));

    if (!data || !data.success) {
      console.warn("[checkin] attendee lookup failed:", data);
      showResult(
        "error",
        "Attendee Not Found",
        data?.error || `No attendee found with ID: ${attendeeId}`,
        { id: attendeeId },
      );
      return;
    }

    const attendee = data.data.attendee;
    const isCheckedIn = data.data.is_checked_in;
    const isApproved = data.data.is_approved;
    const isInPerson = data.data.is_in_person;
    const participationType = data.data.participation_type || "Unknown";

    console.log(
      "[checkin] attendee:",
      attendee.name,
      "| checked_in:",
      isCheckedIn,
      "| approved:",
      isApproved,
      "| in_person:",
      isInPerson,
      "| participation:",
      participationType,
    );

    // Block: already checked in (applies to both modes)
    if (isCheckedIn) {
      console.log("[checkin] attendee already checked in");
      showResult(
        "warning",
        "Already Checked In",
        `${attendee.name} was already checked in.`,
        {
          name: attendee.name,
          email: attendee.email,
          ticket: attendee.ticket_name,
          participationType: participationType,
          checkedInAt: attendee.checked_in_at,
        },
      );
      return;
    }

    // Block: not approved
    if (!isApproved) {
      console.log(
        "[checkin] attendee not approved, status:",
        attendee.approval_status,
      );
      showResult(
        "error",
        "Not Approved",
        `${attendee.name} has not been approved for check-in.`,
        {
          name: attendee.name,
          email: attendee.email,
          participationType: participationType,
          status: attendee.approval_status,
        },
      );
      return;
    }

    // Block: not In-Person
    if (!isInPerson) {
      console.log("[checkin] attendee not In-Person:", participationType);
      showResult(
        "error",
        "Online Attendee",
        `${attendee.name} is registered as "${participationType}" and cannot be checked in at the physical event.`,
        {
          name: attendee.name,
          email: attendee.email,
          ticket: attendee.ticket_name,
          participationType: participationType,
        },
      );
      return;
    }

    // Determine mode
    const mode = getCheckInMode();

    if (mode === "preview") {
      // Preview mode: show attendee info and wait for confirmation
      showPreview(attendee, participationType);
      return;
    }

    // Instant mode: check in immediately
    await performCheckIn(attendeeId, attendee);
  } catch (err) {
    console.error("[checkin] caught error:", err);
    showResult(
      "error",
      "Error",
      err.message || "An unexpected error occurred.",
      { id: attendeeId },
    );
  } finally {
    isCheckingIn = false;
    console.log("[checkin] processScanResult done");
  }
}

/**
 * Show preview panel with attendee info before confirming check-in.
 */
function showPreview(attendee, participationType) {
  hideAllPanels();

  // Store pending data for confirm
  pendingAttendeeId = attendee.api_id;
  pendingAttendeeData = attendee;

  const previewPanel = document.getElementById("previewPanel");
  const previewName = document.getElementById("previewName");
  const previewEmail = document.getElementById("previewEmail");
  const previewTicket = document.getElementById("previewTicket");
  const previewParticipation = document.getElementById("previewParticipation");
  const previewStatus = document.getElementById("previewStatus");
  const previewId = document.getElementById("previewId");

  if (previewName) previewName.textContent = attendee.name || "Unknown";
  if (previewEmail) previewEmail.textContent = attendee.email || "-";
  if (previewTicket) previewTicket.textContent = attendee.ticket_name || "-";

  if (previewParticipation) {
    previewParticipation.textContent = participationType || "Unknown";
    const isInPerson = (participationType || "").toLowerCase() === "in-person";
    previewParticipation.style.color = isInPerson
      ? "var(--success)"
      : "var(--warning)";
  }

  if (previewStatus) {
    previewStatus.textContent = attendee.approval_status || "Unknown";
    previewStatus.style.color =
      attendee.approval_status === "approved" ||
      attendee.approval_status === "checked_in"
        ? "var(--success)"
        : "var(--warning)";
  }

  if (previewId) previewId.textContent = attendee.api_id || "-";

  if (previewPanel) previewPanel.classList.remove("hidden");

  console.log("[checkin] preview shown for:", attendee.name);
}

/**
 * Confirm check-in after preview. Called by the "Confirm Check-In" button.
 */
async function confirmCheckIn() {
  if (!pendingAttendeeId || !pendingAttendeeData) {
    console.error("[checkin] confirmCheckIn called but no pending data");
    showToast("No attendee to check in", "error");
    return;
  }

  const attendeeId = pendingAttendeeId;
  const attendee = pendingAttendeeData;

  // Clear pending state
  pendingAttendeeId = null;
  pendingAttendeeData = null;

  try {
    await performCheckIn(attendeeId, attendee);
  } catch (err) {
    console.error("[checkin] confirmCheckIn error:", err);
    showResult(
      "error",
      "Error",
      err.message || "An unexpected error occurred.",
      { id: attendeeId },
    );
  }
}

/**
 * Cancel the preview and go back to scanning/manual entry.
 */
function cancelPreview() {
  pendingAttendeeId = null;
  pendingAttendeeData = null;
  isCheckingIn = false;

  const previewPanel = document.getElementById("previewPanel");
  if (previewPanel) previewPanel.classList.add("hidden");

  const manualInput = document.getElementById("manualIdInput");
  if (manualInput) manualInput.value = "";

  console.log("[checkin] preview cancelled");
}

/**
 * Perform the actual check-in API call and show the result.
 */
async function performCheckIn(attendeeId, attendee) {
  showLoading(`Checking in ${attendee.name}...`);
  console.log("[checkin] calling api.checkIn:", attendeeId);

  const checkInData = await api.checkIn(attendeeId);
  console.log("[checkin] checkIn response:", JSON.stringify(checkInData));

  if (!checkInData || !checkInData.success) {
    console.warn("[checkin] check-in API failed:", checkInData);
    showResult(
      "error",
      "Check-In Failed",
      checkInData?.error || "An error occurred during check-in.",
      {
        name: attendee.name,
        email: attendee.email,
      },
    );
    return;
  }

  console.log("[checkin] check-in successful, showing result");
  showResult("success", "Checked In!", checkInData.data.message, {
    name: checkInData.data.name,
    apiId: checkInData.data.api_id,
    checkedInAt: checkInData.data.checked_in_at,
  });

  showToast(`${attendee.name} checked in successfully!`, "success");
}

// ===== UI Updates =====

function showLoading(message) {
  hideAllPanels();

  const loadingEl = document.getElementById("loadingPanel");
  const loadingText = document.getElementById("loadingText");
  if (loadingEl) loadingEl.classList.remove("hidden");
  if (loadingText) loadingText.textContent = message || "Loading...";
}

function showResult(type, title, message, details = {}) {
  hideAllPanels();

  const resultPanel = document.getElementById("resultPanel");
  const resultContainer = document.getElementById("resultContainer");
  const resultTitle = document.getElementById("resultTitle");
  const resultMessage = document.getElementById("resultMessage");
  const resultDetails = document.getElementById("resultDetails");

  if (!resultPanel || !resultContainer) {
    console.error(
      "[ui] missing DOM elements: resultPanel=",
      !!resultPanel,
      "resultContainer=",
      !!resultContainer,
    );
    return;
  }

  console.log("[ui] showResult:", type, title, message);
  resultContainer.className = `result-${type}`;

  const iconMap = {
    success: "&#10004;&#65039;",
    error: "&#10060;",
    warning: "&#9888;&#65039;",
  };
  const iconEl = document.getElementById("resultIcon");
  if (iconEl) iconEl.innerHTML = iconMap[type] || iconMap.error;

  if (resultTitle) resultTitle.textContent = title;
  if (resultMessage) resultMessage.textContent = message;

  if (resultDetails) {
    let detailsHtml = "";

    if (details.name)
      detailsHtml += `<p><strong>Name:</strong> ${escapeHtml(details.name)}</p>`;
    if (details.email)
      detailsHtml += `<p><strong>Email:</strong> ${escapeHtml(details.email)}</p>`;
    if (details.ticket)
      detailsHtml += `<p><strong>Ticket:</strong> ${escapeHtml(details.ticket)}</p>`;
    if (details.participationType)
      detailsHtml += `<p><strong>Participation:</strong> ${escapeHtml(details.participationType)}</p>`;
    if (details.checkedInAt) {
      detailsHtml += `<p><strong>Checked in:</strong> ${formatTimestamp(details.checkedInAt)}</p>`;
    }
    if (details.status)
      detailsHtml += `<p><strong>Status:</strong> ${escapeHtml(details.status)}</p>`;
    if (details.id)
      detailsHtml += `<p><strong>ID:</strong> ${escapeHtml(details.id)}</p>`;
    if (details.apiId)
      detailsHtml += `<p><strong>ID:</strong> ${escapeHtml(details.apiId)}</p>`;
    if (details.rawValue) {
      detailsHtml += `<p class="text-muted" style="font-size:0.8rem;word-break:break-all"><strong>Raw:</strong> ${escapeHtml(details.rawValue)}</p>`;
    }

    resultDetails.innerHTML = detailsHtml;
  }

  resultPanel.classList.remove("hidden");
  console.log("[ui] resultPanel shown, classes:", resultPanel.className);

  if (type === "success") {
    playSuccessSound();
  }
}

function hideAllPanels() {
  ["loadingPanel", "resultPanel", "previewPanel"].forEach((id) => {
    const el = document.getElementById(id);
    if (el) el.classList.add("hidden");
  });
}

function resetScanner() {
  hideAllPanels();
  lastScannedId = null;
  isCheckingIn = false;
  pendingAttendeeId = null;
  pendingAttendeeData = null;

  const scannerArea = document.getElementById("scannerArea");
  if (scannerArea) scannerArea.classList.remove("hidden");

  const manualInput = document.getElementById("manualIdInput");
  if (manualInput) manualInput.value = "";

  if (isScanning) {
    startScanning();
  } else {
    const startBtn = document.getElementById("startScanBtn");
    if (startBtn) startBtn.classList.remove("hidden");
  }
}

function showManualOnly() {
  const scannerArea = document.getElementById("scannerArea");
  if (scannerArea) scannerArea.classList.add("hidden");

  const startBtn = document.getElementById("startScanBtn");
  const stopBtn = document.getElementById("stopScanBtn");
  if (startBtn) startBtn.classList.add("hidden");
  if (stopBtn) stopBtn.classList.add("hidden");
}

// ===== Sound Effects =====

function playSuccessSound() {
  try {
    const audioContext = new (
      window.AudioContext || window.webkitAudioContext
    )();
    const oscillator = audioContext.createOscillator();
    const gainNode = audioContext.createGain();

    oscillator.connect(gainNode);
    gainNode.connect(audioContext.destination);

    oscillator.frequency.setValueAtTime(800, audioContext.currentTime);
    oscillator.frequency.setValueAtTime(1000, audioContext.currentTime + 0.1);
    oscillator.frequency.setValueAtTime(1200, audioContext.currentTime + 0.2);

    gainNode.gain.setValueAtTime(0.3, audioContext.currentTime);
    gainNode.gain.exponentialRampToValueAtTime(
      0.01,
      audioContext.currentTime + 0.4,
    );

    oscillator.start(audioContext.currentTime);
    oscillator.stop(audioContext.currentTime + 0.4);
  } catch {
    // Silently fail if audio not available
  }
}

// ===== Utility =====

function escapeHtml(text) {
  if (!text) return "";
  const div = document.createElement("div");
  div.appendChild(document.createTextNode(text));
  return div.innerHTML;
}
