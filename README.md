goldfish (`gf') is a IPC file finder.

---

You can search by filename with `q:your query`. `c: Exit` to quit.

Goldfish is intentionally barebones to support use as a subprocess in a graphical application.
The default setup is similar to running `fd . | fzf` with less pipes to handle. One might even say it's just a nucleo & ignore wrapper, because I wanted `fd . | fzf` outside the terminal.


##  Unlike most fuzzy matchers...
 - Input is stdin and expects a new line.
 - Most recent results list is printed to stdout
 - There is no "return value".

## Plans (assuming future progress):
 - convert current cli arguments to `c:` commands
 - support searching inside files (as seen in [Using fzf as interactive Ripgrep launcher](https://github.com/junegunn/fzf/blob/master/ADVANCED.md#using-fzf-as-interactive-ripgrep-launcher))
