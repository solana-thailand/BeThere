/**
 * BeThere Attendee Registration Template Generator
 * Compatible with: event-checkin-domain v0.1.0 (32-column layout)
 *
 * Column Layout (A–AF, 32 columns):
 *   Section 1 — Identity (A–E):        api_id, name, first_name, last_name, email
 *   Section 2 — Registration (F–I):    ticket_name, registration_date, approval_status, participation_type
 *   Section 3 — Contact (J–L):         phone, contact_channel, contact_handle
 *   Section 4 — Deposit (M–Q):         deposit_agreed, deposit_method, deposit_amount, deposit_tx_signature, deposit_verified
 *   Section 5 — Lifecycle (R–X):       checked_in_at, checked_in_by, solana_address, qr_code_url, claim_token, claimed_at, nft_proof_url
 *   Section 6 — Bank & Refund (Y–AD):  bank_account, bank_name, account_name, refund_status, refund_link, send_email_status
 *   Section 7 — Consent (AE–AF):       consent_given, photo_consent
 *
 * Usage: Open Google Sheets → Extensions → Apps Script → Paste → Run
 */
function createBeThereTemplate() {
  var ss = SpreadsheetApp.getActiveSpreadsheet();
  var sheet = ss.getActiveSheet();

  sheet.setName("Attendees");

  // Clear all data, formats, and validations
  sheet.clear();

  // ── Column definitions (keeps headers, widths, dropdowns in sync) ──────
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
  ];

  var TOTAL = COLUMNS.length; // 32
  var LAST_COL_LETTER = columnLetter(TOTAL - 1); // "AF"
  var DATA_ROWS = 999;

  // ── 1. Headers ─────────────────────────────────────────────────────────
  var headers = COLUMNS.map(function(c) { return c.header; });
  sheet.getRange(1, 1, 1, TOTAL).setValues([headers]);

  var headerRange = sheet.getRange(1, 1, 1, TOTAL);
  headerRange.setFontWeight("bold")
             .setBackground("#F3F4F6")
             .setVerticalAlignment("middle")
             .setHorizontalAlignment("center");
  sheet.setFrozenRows(1);

  // ── 2. Column widths ───────────────────────────────────────────────────
  for (var i = 0; i < TOTAL; i++) {
    if (COLUMNS[i].width) {
      sheet.setColumnWidth(i + 1, COLUMNS[i].width);
    }
  }

  // ── 3. Dropdowns (Data Validation) ─────────────────────────────────────
  for (var i = 0; i < TOTAL; i++) {
    if (COLUMNS[i].dropdown) {
      setDropdown(sheet, i + 1, COLUMNS[i].dropdown, DATA_ROWS);
    }
  }

  // ── 4. Conditional Formatting ──────────────────────────────────────────
  var rules = [];

  // Approval status colors
  rules.push(condRule(sheet, "H2:H1000", "Approved",        { fontColor: "#34A853" }));
  rules.push(condRule(sheet, "H2:H1000", "PendingApproval", { fontColor: "#FBBC04" }));
  rules.push(condRule(sheet, "H2:H1000", "CheckedIn",       { fontColor: "#4285F4" }));

  // Deposit/verified highlight
  rules.push(condRule(sheet, "M2:M1000", "Yes", { background: "#D9EAD3" })); // deposit_agreed
  rules.push(condRule(sheet, "Q2:Q1000", "Yes", { background: "#D9EAD3" })); // deposit_verified

  // Refund highlight
  rules.push(condRule(sheet, "AB2:AB1000", "refunded", { background: "#D9EAD3" }));

  // Consent highlight
  rules.push(condRule(sheet, "AE2:AE1000", "Yes", { background: "#D9EAD3" })); // consent_given
  rules.push(condRule(sheet, "AF2:AF1000", "Yes", { background: "#D9EAD3" })); // photo_consent

  sheet.setConditionalFormatRules(rules);

  // ── 5. Section background tinting ──────────────────────────────────────
  // Lifecycle section (R–X) → light yellow
  tintSection(sheet, 18, 24, "#FFF2CC"); // cols 18–24 = R–X
  // Bank & Refund (Y–AD) → light blue
  tintSection(sheet, 25, 30, "#E8F0FE"); // cols 25–30 = Y–AD
  // Consent (AE–AF) → light green
  tintSection(sheet, 31, 32, "#E6F4EA"); // cols 31–32 = AE–AF

  // ── 6. Example row ─────────────────────────────────────────────────────
  var exampleRow = [
    "0192a3b4-c5d6-7e8f-9a0b-1c2d3e4f5a6b", // A:  api_id (UUIDv7)
    "Somchai Wattana",                         // B:  name
    "Somchai",                                 // C:  first_name
    "Wattana",                                 // D:  last_name
    "somchai@email.com",                       // E:  email
    "Self-Registered",                         // F:  ticket_name
    "2026-05-20T10:30:00+07:00",              // G:  registration_date
    "Approved",                                // H:  approval_status
    "In-Person",                               // I:  participation_type
    "0812345678",                              // J:  phone
    "Telegram",                                // K:  contact_channel
    "@somchai_tg",                             // L:  contact_handle
    "Yes",                                     // M:  deposit_agreed
    "thb",                                     // N:  deposit_method
    "500",                                     // O:  deposit_amount
    "PP-REF12345",                             // P:  deposit_tx_signature
    "Yes",                                     // Q:  deposit_verified
    "",                                        // R:  checked_in_at
    "",                                        // S:  checked_in_by
    "7xKXtg2CW87d97TXJSDpbD5jBkheTqA85T",     // T:  solana_address
    "https://bethere.app/e/bkk2026?q=gst-abc", // U:  qr_code_url
    "0192claim-token-12345",                   // V:  claim_token
    "",                                        // W:  claimed_at
    "",                                        // X:  nft_proof_url
    "",                                        // Y:  bank_account
    "",                                        // Z:  bank_name
    "",                                        // AA: account_name
    "",                                        // AB: refund_status
    "",                                        // AC: refund_link
    "",                                        // AD: send_email_status
    "Yes",                                     // AE: consent_given
    "No",                                      // AF: photo_consent
  ];
  sheet.getRange(2, 1, 1, TOTAL).setValues([exampleRow]);

  SpreadsheetApp.getUi().alert(
    "BeThere Template Created!\n\n" +
    TOTAL + " columns (A–" + LAST_COL_LETTER + ")\n" +
    "Sheet: \"Attendees\"\n" +
    "Dropdowns, conditional formatting, and example data included."
  );
}

// ── Helpers ───────────────────────────────────────────────────────────────

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
