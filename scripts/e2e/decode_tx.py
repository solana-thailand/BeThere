#!/usr/bin/env python3
"""Decode a serialized Solana transaction from the BeThere worker."""
import base64, json, urllib.request, sys

def base58_encode(data: bytes) -> str:
    alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
    n = int.from_bytes(data, 'big')
    result = ''
    while n > 0:
        n, r = divmod(n, 58)
        result = alphabet[r] + result
    for byte in data:
        if byte == 0:
            result = '1' + result
        else:
            break
    return result

KNOWN = {
    "9Bz7p4RWdX7eaR4hFUeCc7aSZjDHsie8q1u8imwavkBN": "organizer",
    "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU": "usdc_mint",
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA": "token_program",
    "11111111111111111111111111111111": "system_program",
    "SysvarRent111111111111111111111111111111111": "rent_sysvar",
    "2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo": "escrow_program",
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL": "ata_program",
}

def decode_compact_u16(data, pos):
    val = 0
    shift = 0
    while True:
        byte = data[pos]
        pos += 1
        val |= (byte & 0x7f) << shift
        if byte & 0x80 == 0:
            break
        shift += 7
    return val, pos

# Get TX from worker
req = urllib.request.Request(
    "http://localhost:8787/api/escrow/create-event",
    data=json.dumps({"event_id": "default"}).encode(),
    headers={
        "Content-Type": "application/json",
        "Authorization": "Bearer dev-token"
    }
)

resp = urllib.request.urlopen(req)
result = json.loads(resp.read())
tx_b64 = result["data"]["transaction"]
escrow_addr = result["data"]["escrow_address"]
on_chain_id = result["data"]["on_chain_event_id"]

tx_bytes = base64.b64decode(tx_b64)
print(f"TX size: {len(tx_bytes)} bytes")
print(f"Escrow PDA: {escrow_addr}")
print(f"On-chain event ID: {on_chain_id}")

pos = 0

# Signature count
sig_count, pos = decode_compact_u16(tx_bytes, pos)
print(f"\nSignature count: {sig_count}")

for i in range(sig_count):
    sig = tx_bytes[pos:pos+64]
    print(f"  Signature {i}: all zeros = {all(b == 0 for b in sig)}")
    pos += 64

# Message header
num_required_signatures = tx_bytes[pos]
num_readonly_signed = tx_bytes[pos+1]
num_readonly_unsigned = tx_bytes[pos+2]
pos += 3

print(f"\nMessage header:")
print(f"  num_required_signatures: {num_required_signatures}")
print(f"  num_readonly_signed:     {num_readonly_signed}")
print(f"  num_readonly_unsigned:   {num_readonly_unsigned}")

# Account keys count
acct_count, pos = decode_compact_u16(tx_bytes, pos)
print(f"  num_account_keys:        {acct_count}")

# Account keys
accounts = []
for i in range(acct_count):
    key_bytes = tx_bytes[pos:pos+32]
    b58 = base58_encode(key_bytes)
    accounts.append(b58)
    pos += 32

# Determine roles
for i, addr in enumerate(accounts):
    if i < num_required_signatures:
        if i < num_required_signatures - num_readonly_signed:
            role = "signer+writable"
        else:
            role = "signer+readonly"
    elif i < acct_count - num_readonly_unsigned:
        role = "non-signer+writable"
    else:
        role = "non-signer+readonly"

    label = KNOWN.get(addr, "")
    if addr == escrow_addr:
        label = "event_escrow"

    print(f"  [{i}] {addr} ({role}) {f'← {label}' if label else ''}")

# Recent blockhash
blockhash = base58_encode(tx_bytes[pos:pos+32])
pos += 32
print(f"\nRecent blockhash: {blockhash}")

# Instructions
ix_count, pos = decode_compact_u16(tx_bytes, pos)
print(f"\nInstructions: {ix_count}")

for i in range(ix_count):
    program_idx, pos = decode_compact_u16(tx_bytes, pos)
    acct_len, pos = decode_compact_u16(tx_bytes, pos)
    acct_indices = list(tx_bytes[pos:pos+acct_len])
    pos += acct_len
    data_len, pos = decode_compact_u16(tx_bytes, pos)
    ix_data = tx_bytes[pos:pos+data_len]
    pos += data_len

    discriminator = ix_data[0] if ix_data else -1

    print(f"  Instruction {i}:")
    print(f"    Program: account [{program_idx}] = {accounts[program_idx][:12]}...")
    print(f"    Accounts ({len(acct_indices)}): {acct_indices}")
    for idx in acct_indices:
        addr = accounts[idx]
        label = KNOWN.get(addr, "")
        if addr == escrow_addr:
            label = "event_escrow"
        print(f"      [{idx}] {addr[:16]}... {f'({label})' if label else ''}")
    print(f"    Data ({data_len} bytes): discriminator={discriminator}")

    # Decode instruction data
    if len(ix_data) >= 9:
        event_id = int.from_bytes(ix_data[1:9], 'little')
        deposit_amt = int.from_bytes(ix_data[9:17], 'little') if len(ix_data) >= 17 else "?"
        print(f"    event_id={event_id}, deposit_amount={deposit_amt}")
