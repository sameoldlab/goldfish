goldfish (`gf') is a IPC file finder.

---

You can search by filename with `q:your query`. `c: Exit` to quit.

Goldfish is intentionally barebones to support use as a subprocess in a graphical application.
The default setup is similar to running `fd . | fzf` with less pipes to handle.

Plans (may or may not happen... ever):
 - convert current cli arguments to `c:` commands
 - support searching inside files (like ripgrep)
