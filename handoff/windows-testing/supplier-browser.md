# Windows test: the supplier browser (PR 1864)

For Tadjiro, on the work PC, with the newest build (close KoperCAD before updating). About 15 minutes. Nothing here needs admin rights. The built-in browser has only been type-checked for Windows; this is its first real run. Report any problem with Help > Copy problem report, plus the numbered step.

## 0. Is WebView2 there?
Press Win, type "Apps & features", look for **Microsoft Edge WebView2 Runtime**. Windows 11 and current Windows 10 have it. If it is missing, step 1 should still work through your normal browser (that is test 6).

## 1. Open it
Parts window (Ctrl+K, type Parts). Click **Get parts from supplier**, pick **WAGO CAD models** (the only address confirmed so far).
Expect: a second KoperCAD window opens with the WAGO CAD portal; its title bar says "WAGO CAD models (wago.partcommunity.com) - KoperCAD". In the Parts window a line "WAGO CAD models is open. Files taken: 0" with Stop.
Fail if: nothing opens, or the window is blank or shows an error page, or an orange note about WebView2 appears although it is installed.

## 2. Sign in and download
Sign in on the portal yourself, find a WAGO article you own (for example one from your EPLAN parts list), press the CAD download button, choose STEP.
Expect: the title bar says "Downloading ..." then "Saved. Parts takes it in the KoperCAD window". Within about two seconds "Files taken: 1" and the part window reads the file (the part matcher says which part it fits or asks).
Fail if: the download never starts, the file ends up in Downloads instead, or it is taken for the wrong part without a reason shown.
Look in: `%LOCALAPPDATA%\KoperCAD\browser\landing\wago\` (menu: Get parts from supplier > Downloads folder > WAGO). The file should be there while it is taken (a ".part" ending only while it downloads).

## 3. Sign-in is remembered
Close the supplier window, open WAGO again from the menu.
Expect: still signed in (cookies live in `%LOCALAPPDATA%\KoperCAD\browser\profile\wago\`). Then Get parts from supplier > **Forget sign-in** > WAGO, open it again: signed out.

## 4. Refuses what it should
On any portal try to download something that is not a model (a PDF data sheet or an .exe).
Expect: title bar "Not kept: ... is not a file type Parts can take"; nothing in the landing folder.

## 5. Other suppliers (addresses not confirmed)
Try Rittal PARTcommunity, TraceParts, Siemens Industry Mall, Phoenix Contact, Schneider Electric. For each, write down: did the page open, is it the right place for CAD downloads, did the download land. Wrong or dead addresses can be corrected without a new build: edit `%LOCALAPPDATA%\KoperCAD\browser\suppliers.json` (see below), or send me the right address.
Example suppliers.json (replaces an entry with the same id, adds new ones, "hidden": true removes one):
```
[{"id":"rittal","name":"Rittal PARTcommunity","start_url":"https://rittal.partcommunity.com/3d-cad-models/","accepts":["step","dxf","zip"]}]
```

## 6. Fallback
Menu > tick **Use my normal browser**, open WAGO again.
Expect: your normal browser opens the portal; Downloads is watched for 30 minutes; a STEP/ZIP you save there is offered to Parts. The "?" next to the menu explains it. Pressing **Stop** removes the strip and any orange note.

## 7. The existing buttons
Select a WAGO or Schneider part, press **Import** or **Get CAD from TraceParts**: the supplier window should open at that supplier (the part number is in the search address when the supplier has one).

## 8. Personal models stay out of shared bundles
After a download was taken (step 2): Parts > More > **Export library** with the "share" kind and CAD files ticked.
Expect: the WAGO file is left out with a note "models from CADENAS PARTcommunity are for your own use". A backup export keeps it. (This is the rule from PR 1862; it has not been tried on a real window download.)

## What to tell me
For each numbered step: works / fails / not tried, the supplier addresses that were wrong, and whether closing and reopening the window felt fine. Known open points: the window has no address bar or back button yet (use Alt+Left and F5), password autofill of the web view is not switched off yet, and the address of a window download is still shown as a link (follow-up).
