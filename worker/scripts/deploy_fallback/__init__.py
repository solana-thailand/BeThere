"""Generators for the production PUT API deploy fallback.

`worker/deploy.sh` used to embed these programs inside `python3 -c "..."`
arguments. Shell expansion stays active inside a double-quoted string, so
backticks in the Python *comments* ran as commands before Python started —
one of them was a literal `wrangler deploy`. See `.issues/065`.

Everything here is plain tracked source: no shell quoting, no interpolation
of shell variables into Python text, and no secret on the command line.
"""
