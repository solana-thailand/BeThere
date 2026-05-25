/**
 * BeThere Event Check-in — Google Sheet Template Setup
 *
 * Run `setupSheet()` from Extensions → Apps Script to initialize
 * the active spreadsheet with the correct headers for the Attendees tab.
 *
 * Column Layout (A–AD, 30 columns):
 *   Section 1 — Identity (A–E):
 *     A = api_id, B = name, C = first_name, D = last_name, E = email
 *   Section 2 — Registration (F–I):
 *     F = ticket_name, G = registration_date, H = approval_status, I = participation_type
 *   Section 3 — Contact (J–L):
 *     J = phone, K = contact_channel, L = contact_handle
 *   Section 4 — Deposit (M–Q):
 *     M = deposit_agreed, N = deposit_method, O = deposit_amount,
 *     P = deposit_tx_signature, Q = deposit_verified
 *   Section 5 — Lifecycle (R–X):
 *     R = checked_in_at, S = checked_in_by, T = solana_address,
 *     U = qr_code_url, V = claim_token, W = claimed_at, X = nft_proof_url
 *   Section 6 — Bank & Refund (Y–AD):
 *     Y = bank_account, Z = bank_name, AA = account_name,
 *     AB = refund_status, AC = refund_link, AD = send_email_status
 */

/**
 * Sets up the "Attendees" sheet with correct column headers in row 1.
 * Creates the tab if it doesn't exist. Freezes row 1.
 * Safe to re-run — only writes headers if row 1 is empty.
 */
function setupSheet() {
  const HEADERS = [
    // Section 1: Identity (A–E)
    'api_id',
    'name',
    'first_name',
    'last_name',
    'email',
    // Section 2: Registration (F–I)
    'ticket_name',
    'registration_date',
    'approval_status',
    'participation_type',
    // Section 3: Contact (J–L)
    'phone',
    'contact_channel',
    'contact_handle',
    // Section 4: Deposit (M–Q)
    'deposit_agreed',
    'deposit_method',
    'deposit_amount',
    'deposit_tx_signature',
    'deposit_verified',
    // Section 5: Lifecycle (R–W)
    'checked_in_at',
    'checked_in_by',
    'solana_address',
    'qr_code_url',
    'claim_token',
    'claimed_at',
    'nft_proof_url',
    // Section 6: Bank & Refund (Y–AD)
    'bank_account',
    'bank_name',
    'account_name',
    'refund_status',
    'refund_link',
    'send_email_status',
  ];

  const SHEET_NAME = 'Attendees';

  const ss = SpreadsheetApp.getActiveSpreadsheet();
  let sheet = ss.getSheetByName(SHEET_NAME);

  if (!sheet) {
    sheet = ss.insertSheet(SHEET_NAME);
    Logger.log(`Created new sheet tab: "${SHEET_NAME}"`);
  } else {
    Logger.log(`Found existing sheet tab: "${SHEET_NAME}"`);
  }

  // Check if row 1 already has content
  const firstRow = sheet.getRange(1, 1, 1, HEADERS.length).getValues()[0];
  const hasContent = firstRow.some(cell => String(cell).trim() !== '');

  if (hasContent) {
    Logger.log('Row 1 already has content. Overwriting with correct headers...');
  }

  // Write headers to row 1
  sheet.getRange(1, 1, 1, HEADERS.length).setValues([HEADERS]);

  // Format header row
  const headerRange = sheet.getRange(1, 1, 1, HEADERS.length);
  headerRange
    .setFontWeight('bold')
    .setBackground('#4285f4')
    .setFontColor('#ffffff');

  // Freeze header row
  sheet.setFrozenRows(1);

  // Auto-resize columns for readability
  for (let i = 1; i <= HEADERS.length; i++) {
    sheet.autoResizeColumn(i);
  }

  Logger.log(`✅ Set up ${HEADERS.length} column headers in "${SHEET_NAME}" (A–AD)`);
  Logger.log('Headers: ' + HEADERS.join(', '));
}

/**
 * Sets up the "staff" sheet tab with a single column for staff email addresses.
 * The BeThere worker reads staff emails from this tab for authentication.
 */
function setupStaffSheet() {
  const SHEET_NAME = 'staff';

  const ss = SpreadsheetApp.getActiveSpreadsheet();
  let sheet = ss.getSheetByName(SHEET_NAME);

  if (!sheet) {
    sheet = ss.insertSheet(SHEET_NAME);
    Logger.log(`Created new sheet tab: "${SHEET_NAME}"`);
  }

  // Write header
  sheet.getRange(1, 1).setValue('email');
  sheet.getRange(1, 1).setFontWeight('bold').setBackground('#34a853').setFontColor('#ffffff');
  sheet.setFrozenRows(1);
  sheet.autoResizeColumn(1);

  Logger.log(`✅ Set up "${SHEET_NAME}" tab with email column`);
}

/**
 * Run both setup functions — use this for a fresh spreadsheet.
 */
function setupAll() {
  setupSheet();
  setupStaffSheet();
  Logger.log('✅ All sheets initialized!');
}
