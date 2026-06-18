/**
 * BeThere Attendee Registration Template Generator
 * Compatible with: event-checkin-domain v0.1.0 (33-column layout)
 *
 * Column Layout (A–AG, 33 columns):
 *   Section 1 — Identity (A–E):        api_id, name, first_name, last_name, email
 *   Section 2 — Registration (F–I):    ticket_name, registration_date, approval_status, participation_type
 *   Section 3 — Contact (J–L):         phone, contact_channel, contact_handle
 *   Section 4 — Deposit (M–Q):         deposit_agreed, deposit_method, deposit_amount, deposit_tx_signature, deposit_verified
 *   Section 5 — Lifecycle (R–X):       checked_in_at, checked_in_by, solana_address, qr_code_url, claim_token, claimed_at, nft_proof_url
 *   Section 6 — Bank & Refund (Y–AD):  bank_account, bank_name, account_name, refund_status, refund_link, send_email_status
 *   Section 7 — Consent (AE–AG):       consent_given, photo_consent, consent_marketing
 *
 * Idempotent: safe to re-run. Only updates missing/changed headers,
 * never deletes data rows. Re-applies formatting, dropdowns, and permissions.
 *
 * Usage: Open Google Sheets → Extensions → Apps Script → Paste → Run
 */

/**
 * Reads the service account email from Script Properties.
 * Set it once via: Project Settings → Script Properties → SERVICE_ACCOUNT_EMAIL
 */
function getServiceAccountEmail() {
  var email = PropertiesService.getScriptProperties().getProperty("SERVICE_ACCOUNT_EMAIL");
  if (!email) {
    Logger.log("⚠️ SERVICE_ACCOUNT_EMAIL not set in Script Properties.");
    Logger.log("Set it via: Project Settings → Script Properties → add SERVICE_ACCOUNT_EMAIL");
  }
  return email;
}

function createBeThereTemplate() {
  var ss = SpreadsheetApp.getActiveSpreadsheet();

  // ── 0. Grant service account editor access ─────────────────────────────
  var serviceEmail = getServiceAccountEmail();
  if (serviceEmail) {
    grantEditorAccess(ss, serviceEmail);
  }

  // ── 1. Attendees tab ───────────────────────────────────────────────────
  setupAttendeesTab(ss);

  // ── 2. Staff tab ───────────────────────────────────────────────────────
  setupStaffTab(ss);

  var statusMsg = "BeThere Template Ready!\n\n" +
    "Attendees: 33 columns (A–AG)\n" +
    "Staff: email + role columns";
  if (serviceEmail) {
    statusMsg += "\nService account: " + serviceEmail + " (editor)";
  } else {
    statusMsg += "\n⚠️ SERVICE_ACCOUNT_EMAIL not set — grant access manually.";
  }
  statusMsg += "\n\nSafe to re-run — existing data is preserved.";

  // ── 3. Surface the admin values (sheet_id + make-a-copy link) ──────────
  var sheetId = ss.getId();
  // Edit URL → copy URL. Anyone with this link gets a one-click copy of the
  // full template structure (columns, formatting, dropdowns all carried over).
  var copyUrl = ss.getUrl().replace(/\/edit.*$/i, "") + "/copy";
  statusMsg += "\n\n──────── FOR THE ADMIN (save these) ────────";
  statusMsg += "\nSpreadsheet ID:\n  " + sheetId;
  statusMsg += "\n\nMake-a-copy link (share view-only with organizers):\n  " + copyUrl;
  statusMsg += "\n\nNote: copies do NOT inherit editors — each organizer must";
  statusMsg += "\nre-share their copy with the service account after copying.";

  SpreadsheetApp.getUi().alert(statusMsg);
}

// ══════════════════════════════════════════════════════════════════════════
// Attendees Tab
// ══════════════════════════════════════════════════════════════════════════

function setupAttendeesTab(ss) {
  var sheet = ss.getSheetByName("Attendees");
  if (!sheet) {
    sheet = ss.insertSheet("Attendees");
  }

  // ── Column definitions ─────────────────────────────────────────────────
  var COLUMNS = [
    { header: "api_id",               width: 150 },
    { header: "name",                 width: 180 },
    { header: "first_name",           width: 120 },
    { header: "last_name",            width: 120 },
    { header: "email",                width: 220 },
    { header: "ticket_name",          width: 130, dropdown: ["Self-Registered", "Walk-In"] },
    { header: "registration_date",    width: 200 },
    { header: "approval_status",      width: 140, dropdown: ["Approved", "PendingApproval", "Invited", "CheckedIn"] },
    { header: "participation_type",   width: 140, dropdown: ["In-Person", "Online"] },
    { header: "phone",                width: 130 },
    { header: "contact_channel",      width: 130, dropdown: ["Telegram", "Line", "Facebook", "X/Twitter"] },
    { header: "contact_handle",       width: 150 },
    { header: "deposit_agreed",       width: 120, dropdown: ["Yes", ""] },
    { header: "deposit_method",       width: 120, dropdown: ["usdc", "thb"] },
    { header: "deposit_amount",       width: 120 },
    { header: "deposit_tx_signature", width: 260 },
    { header: "deposit_verified",     width: 120, dropdown: ["Yes", ""] },
    { header: "checked_in_at",        width: 200 },
    { header: "checked_in_by",        width: 200 },
    { header: "solana_address",       width: 260 },
    { header: "qr_code_url",          width: 260 },
    { header: "claim_token",          width: 200 },
    { header: "claimed_at",           width: 200 },
    { header: "nft_proof_url",        width: 300 },
    { header: "bank_account",         width: 150 },
    { header: "bank_name",            width: 120 },
    { header: "account_name",         width: 150 },
    { header: "refund_status",        width: 130, dropdown: ["", "pending", "refunded", "not_applicable", "failed"] },
    { header: "refund_link",          width: 300 },
    { header: "send_email_status",    width: 140 },
    { header: "consent_given",        width: 120, dropdown: ["Yes", ""] },
    { header: "photo_consent",        width: 120, dropdown: ["Yes", "No", ""] },
    { header: "consent_marketing",     width: 140, dropdown: ["Yes", "No", ""] },
  ];

  var TOTAL = COLUMNS.length; // 33
  var DATA_ROWS = 999;

  // ── Headers (merge: only update missing/changed) ───────────────────────
  var headers = COLUMNS.map(function(c) { return c.header; });
  var existingHeaders = sheet.getRange(1, 1, 1, TOTAL).getValues()[0];
  var headerChanged = false;
  for (var i = 0; i < TOTAL; i++) {
    var existing = String(existingHeaders[i] || "").trim();
    if (existing !== headers[i]) {
      headerChanged = true;
      break;
    }
  }

  if (headerChanged) {
    sheet.getRange(1, 1, 1, TOTAL).setValues([headers]);
  }

  // ── Header formatting (always re-apply) ────────────────────────────────
  var headerRange = sheet.getRange(1, 1, 1, TOTAL);
  headerRange.setFontWeight("bold")
             .setBackground("#F3F4F6")
             .setVerticalAlignment("middle")
             .setHorizontalAlignment("center");
  sheet.setFrozenRows(1);

  // ── Column widths (always re-apply) ────────────────────────────────────
  for (var i = 0; i < TOTAL; i++) {
    if (COLUMNS[i].width) {
      sheet.setColumnWidth(i + 1, COLUMNS[i].width);
    }
  }

  // ── Dropdowns (always re-apply) ────────────────────────────────────────
  for (var i = 0; i < TOTAL; i++) {
    if (COLUMNS[i].dropdown) {
      setDropdown(sheet, i + 1, COLUMNS[i].dropdown, DATA_ROWS);
    }
  }

  // ── Conditional Formatting (always re-apply) ───────────────────────────
  var rules = [];

  // Approval status colors
  rules.push(condRule(sheet, "H2:H1000", "Approved",        { fontColor: "#34A853" }));
  rules.push(condRule(sheet, "H2:H1000", "PendingApproval", { fontColor: "#FBBC04" }));
  rules.push(condRule(sheet, "H2:H1000", "CheckedIn",       { fontColor: "#4285F4" }));
  rules.push(condRule(sheet, "H2:H1000", "Invited",         { fontColor: "#A142F4" }));

  // Deposit/verified highlight
  rules.push(condRule(sheet, "M2:M1000", "Yes", { background: "#D9EAD3" }));
  rules.push(condRule(sheet, "Q2:Q1000", "Yes", { background: "#D9EAD3" }));

  // Refund highlight
  rules.push(condRule(sheet, "AB2:AB1000", "refunded", { background: "#D9EAD3" }));

  // Consent highlight
  rules.push(condRule(sheet, "AE2:AE1000", "Yes", { background: "#D9EAD3" }));
  rules.push(condRule(sheet, "AF2:AF1000", "Yes", { background: "#D9EAD3" }));
  rules.push(condRule(sheet, "AG2:AG1000", "Yes", { background: "#D9EAD3" }));

  sheet.setConditionalFormatRules(rules);

  // ── Section background tinting ─────────────────────────────────────────
  tintSection(sheet, 18, 24, "#FFF2CC"); // R–X: Lifecycle
  tintSection(sheet, 25, 30, "#E8F0FE"); // Y–AD: Bank & Refund
  tintSection(sheet, 31, 33, "#E6F4EA"); // AE–AG: Consent

  // ── Example row (only if row 2 is empty) ───────────────────────────────
  var row2 = sheet.getRange(2, 1, 1, 1).getValue();
  if (!row2 || String(row2).trim() === "") {
    var exampleRow = [
      "0192a3b4-c5d6-7e8f-9a0b-1c2d3e4f5a6b",
      "Somchai Wattana",
      "Somchai",
      "Wattana",
      "somchai@email.com",
      "Self-Registered",
      "2026-05-20T10:30:00+07:00",
      "Approved",
      "In-Person",
      "0812345678",
      "Telegram",
      "@somchai_tg",
      "Yes",
      "thb",
      "500",
      "PP-REF12345",
      "Yes",
      "", "", "",
      "7xKXtg2CW87d97TXJSDpbD5jBkheTqA85T",
      "https://bethere.app/e/bkk2026?q=gst-abc",
      "0192claim-token-12345",
      "", "",
      "", "", "",
      "", "", "",
      "Yes", "No", "",
    ];
    sheet.getRange(2, 1, 1, TOTAL).setValues([exampleRow]);
  }
}

// ══════════════════════════════════════════════════════════════════════════
// Staff Tab
// ══════════════════════════════════════════════════════════════════════════

/**
 * Sets up the "staff" tab with email (A) and role (B) columns.
 * The BeThere worker reads staff emails from this tab for authentication.
 * Valid roles: "admin", "organizer", "staff". Defaults to "staff" if empty.
 * Idempotent: only writes headers if missing, never clears staff data.
 */
function setupStaffTab(ss) {
  var STAFF_NAME = "staff";
  var sheet = ss.getSheetByName(STAFF_NAME);
  var isNew = false;
  if (!sheet) {
    sheet = ss.insertSheet(STAFF_NAME);
    isNew = true;
  }

  // Only write headers if row 1 is empty or mismatched
  var existingHeaders = sheet.getRange(1, 1, 1, 2).getValues()[0];
  var needHeaders = String(existingHeaders[0] || "").trim() !== "email"
                 || String(existingHeaders[1] || "").trim() !== "role";

  if (needHeaders) {
    sheet.getRange(1, 1, 1, 2).setValues([["email", "role"]]);
  }

  // Header formatting (always re-apply)
  var staffHeaderRange = sheet.getRange(1, 1, 1, 2);
  staffHeaderRange.setFontWeight("bold")
                   .setBackground("#34A853")
                   .setFontColor("#ffffff")
                   .setVerticalAlignment("middle")
                   .setHorizontalAlignment("center");
  sheet.setFrozenRows(1);

  // Column widths (always re-apply)
  sheet.setColumnWidth(1, 280);
  sheet.setColumnWidth(2, 100);

  // Role dropdown (always re-apply)
  setDropdown(sheet, 2, ["admin", "organizer", "staff"], 999);

  // Example rows (only if sheet is brand new and row 2 is empty)
  if (isNew) {
    var row2 = sheet.getRange(2, 1, 1, 1).getValue();
    if (!row2 || String(row2).trim() === "") {
      sheet.getRange(2, 1, 2, 2).setValues([
        ["admin@example.com", "admin"],
        ["scanner@example.com", "staff"],
      ]);
    }
  }
}

// ══════════════════════════════════════════════════════════════════════════
// Permissions
// ══════════════════════════════════════════════════════════════════════════

/**
 * Grants editor access to the service account if not already present.
 * Uses Drive API (Advanced Services must be enabled) or falls back to
 * SpreadsheetApp.addEditor().
 */
function grantEditorAccess(ss, email) {
  try {
    // Check if already an editor
    var editors = ss.getEditors();
    for (var i = 0; i < editors.length; i++) {
      if (editors[i].getEmail() === email) {
        return; // Already has access
      }
    }
    ss.addEditor(email);
  } catch (e) {
    Logger.log("⚠️ Could not add editor: " + e.message);
    Logger.log("Add " + email + " as editor manually via Share button.");
  }
}

// ══════════════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════════════

function setDropdown(sheet, col, values, rows) {
  var range = sheet.getRange(2, col, rows, 1);
  var rule = SpreadsheetApp.newDataValidation()
    .requireValueInList(values)
    .setAllowInvalid(false)
    .build();
  range.setDataValidation(rule);
}

function condRule(sheet, a1Range, textValue, style) {
  var builder = SpreadsheetApp.newConditionalFormatRule()
    .whenTextEqualTo(textValue)
    .setRanges([sheet.getRange(a1Range)]);

  if (style.fontColor) builder.setFontColor(style.fontColor);
  if (style.background) builder.setBackground(style.background);

  return builder.build();
}

function tintSection(sheet, startCol, endCol, color) {
  var range = sheet.getRange(2, startCol, 999, endCol - startCol + 1);
  range.setBackground(color);
}

function columnLetter(index) {
  var letter = "";
  var n = index;
  while (n >= 0) {
    letter = String.fromCharCode(65 + (n % 26)) + letter;
    n = Math.floor(n / 26) - 1;
  }
  return letter;
}
