# Drive run hang at 1920x1080 (issue 1958)
Not an app hang: `wait 3000` (600 frames) exceeded the control socket's 60 s read timeout on a slow software renderer. Fixed in novo-cli testuser.rs by sending waits in 50-frame pieces (PR 1961). Evidence: /mnt/project-files/qa/pr1958-bc5af852/. The winch run itself is slow (50 to 110 ms per frame with Hoist in 3D open); making it faster is separate.
