# Trying the Claude assistant with your own API key

About ten minutes. It costs a few cents. Nothing but your key (step 4) is sent until step 6.

You need an Anthropic API key (console.anthropic.com, API keys, create key; it starts with `sk-ant-`)
and some credit on that account. A Claude.ai chat subscription is not an API key.

1. Start KoperCAD and open the example (File > Open > Open example).
2. Open Help > Ask Claude (or Ctrl+K and type "Ask Claude"). Claude is off; the settings are open.
3. Paste the key and press Save key. Only the last four characters are shown afterwards.
4. Press Check key. Expect "Anthropic accepted the key." This costs nothing and sends only the key.
   A wrong key says Anthropic did not accept it.
5. Pick what Claude may read. For this test choose "Whole project" (the example is not secret).
   Tick "I have read this and agree" and press Turn Claude on.
6. Ask: "How many devices and pages does this project have?" Expect a short answer and a line
   such as "Looked at the project outline" or "Read part of the project". Under the box the line
   "Chat so far $x · month $y" should show well under one cent on Sonnet, a few cents at most on Opus.
7. Ask: "Rename the first device to -K99." Expect a Suggested change card that reads in plain words. Nothing has changed yet.
   Press Apply. The device tag changes everywhere; Ctrl+Z puts it back. Try Reject on another one.
8. Narrow the scope to "Nothing from the project" and ask "What did you see before?". Expect the
   note "Scope narrowed, so the earlier chat was cleared" and an answer that has no project data.
9. Set the monthly limit just below the "This month" figure (if it shows $0.00, ask once more
   first) and ask anything. Expect "The monthly budget you set is used up. Raise it in Settings >
   Claude to continue." and nothing sent. Set it back (0 means no limit).
10. Compare "This month" in the window with Usage on console.anthropic.com. They should be close
    (ours is an estimate from list prices). Tell us if they differ by more than about 20 percent.

11. Failure paths (these are tested offline with a scripted transport, so this only confirms the
    real API says the same). Save a wrong key such as `sk-ant-api03-wrong` and press Check key or
    ask anything. Expect "Anthropic did not accept the API key. Check it in Settings > Claude."
    and nothing counted under "This month". Then turn the network off and ask: expect "No internet
    connection to Anthropic" after about six seconds (three tries). Restore both.
12. Datasheet: Help > Ask Claude > Part from a datasheet. Choose a small PDF datasheet. Expect the
    confirmation naming the file and its size, then the review table with a page and the quoted
    words under each value. Untick one, press Add part to project, then Ctrl+Z.

Remove the key afterwards with Remove key if you do not want it kept. It lives in
`%LOCALAPPDATA%\KoperCAD\ai\api-key` (not encrypted).

What to report: anything that errors, any step where the screen and these notes disagree, and
how long step 6 took.
