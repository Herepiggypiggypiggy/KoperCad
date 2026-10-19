2026-10-19 11:10 UTC graphics + layout reader, plan, apply and tests done; trimmed v95: 228 shapes on 17 pages, 255 edge marks left out. Waiting on #1031 merge to open the PR; real layout sheets unverified.

- Review fixes on #1048: coordinates beyond 5000 mm and pages over 60000 entities are dropped and reported; sheets over 4000 objects are skipped with a note; titled plate sizes outside 100..5000 mm fall back to the rail scale; tags are parsed once; a device that cannot be placed is skipped, not fatal.
