# tools/decks — synthetic deck & fixture generators

Generators for the **committed** synthetic decks under
`tests/corpus/{asymmetric,controls,modes}/` (the hand-authored live-gate
families, not the vendored corpus).

- **`gen_midi_decks.py`** — the IEEE123-class "midi" network decks that carry
  the asymmetric / control / protection coverage at scale (deterministic,
  byte-reproducible; committed artifacts). Routes each deck to its family dir,
  including the `pending: true` decks for unported features.
- **`gen_shape_fixtures.py`** — the binary/CSV LoadShape/TShape/PriceShape/
  GrowthShape fixtures for `tests/corpus/modes/inputformat/shape_binfiles/`.

Regenerate deliberately (the decks are committed and oracle-validated); see
`GAPS_PLAN.md` §2.1 for the validation ritual and `TESTING.md` → *Add a corpus
deck*.
