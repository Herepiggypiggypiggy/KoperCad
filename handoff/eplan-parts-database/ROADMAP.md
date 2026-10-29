# Roadmap: EPLAN parts database import

- Done: generic reader for EPLAN `.adb`/`.eod` article files (`novo_interop::eplan::import_eod`), maker alias table (`novo_parts::MakerAliases`), preview with editable maker names, worker-thread read.
- Next: confirm the container layout against a real file on the owner's PC (the reader scans for zlib records, so it should not matter); real ETIM/price fields if the database ever carries them; use the alias table in the table and BMEcat importers.
