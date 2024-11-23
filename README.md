# `convert-nonogram`

`convert-nonogram` is a tool that converts images to nonograms. Currently, it only outputs the widely-used XML-based `webpbn` format and the `olsak` format (with the extension `.g`).

`convert-nonogram` does an exact image-to-nonogram format conversion. If you're looking for something that will take an arbitrary image and make a solveable (black and white) nonogram out of it, you can try [Walter Koster's tool].

[Walter Koster's tool]: https://liacs.leidenuniv.nl/~kosterswa/nono/sjoerd/indexeng.html

## How to use it

I use `convert-nonogram` to evaluate the solvability of nonograms while editing them as an image. You can install it with `cargo install convert-nonogram`.

All images supported by the [image] crate are supported as input, but if you try to create JPEG nonograms, you're going to have a bad time.

[image]: https://crates.io/crates/image

### With `pbnsolve`
This is what I do, since [`pbnsolve`] provides useful information about difficulty. You'll have to download and install it [from a tarball] (and probably edit the `Makefile` to help it find `libxml2` -- under Ubuntu, you'll need to do `sudo apt install libxml2-dev`)

[`pbnsolve`]: https://webpbn.com/pbnsolve.html
[from a tarball]: https://code.google.com/archive/p/pbnsolve/downloads

Then, to evaluate an image, do:

```
convert-nonogram examples/tea.png | pbnsolve -tu
```

(`-t` requests detailed difficulty output, and `-u` requires checking for uniqueness. You can add `-aL` (or `-aE`; I don't fully understand the difference) to stop solving when it's not possible to proceed with "line logic".  You can add `-b` to suppress output of the solved grid, but it's useful when debugging a non-unique nonogram or partially-solvable nonogram. `pbnsolve`'s README file documents its other flags.)


Here's a somewhat tricky nonogram that's solveable with only single-line reasoning. The "Lines processed" (relative to "Lines in puzzle", which is the sum of the width and height) is the best indicator of difficulty, I think:
```
$ convert-nonogram examples/shirt_and_tie.png | pbnsolve -tu
UNIQUE LINE SOLUTION:
.........aaaa..
........a....aa
....aaaaaaa...a
...aa.....aa.ab
..aa.......aabb
..a...........b
..a...a.aaaaa.b
..a...a.a...a.b
.aa..aa.aaaaa.b
.a...aa.a...a.b
.a..aaa.a...a.b
aa..a.a..aaa..b
a...a.a......bb
a...a.a......bb
aaaaa.a......bb
ab.a..a......bb
a..a..a......bb
aaaa..a.......b
......aaaaa....
..........aaaaa
Cells Solved: 300 of 300
Lines in Puzzle: 35
Lines Processed: 149 (400%)
Exhaustive Search: 0 cells in 0 passes
Backtracking: 0 probes, 0 guesses, 0 backtracks
Probe Sequences: 0
Plod cycles: 1, Sprint cycles: 0
Cache Hits: 0/0 (0.0%) Adds: 0  Flushes: 0
Processing Time: 0.000205 sec 
```



Here's the same nonogram with the button on the shirt sleeve removed. Now it requires backtracking to solve:
```
$ convert-nonogram examples/shirt_and_tie_no_button.png | pbnsolve -tu
UNIQUE SOLUTION:
.........aaaa..
........a....aa
....aaaaaaa...a
...aa.....aa.ab
..aa.......aabb
..a...........b
..a...a.aaaaa.b
..a...a.a...a.b
.aa..aa.aaaaa.b
.a...aa.a...a.b
.a..aaa.a...a.b
aa..a.a..aaa..b
a...a.a......bb
a...a.a......bb
aaaaa.a......bb
a..a..a......bb
a..a..a......bb
aaaa..a.......b
......aaaaa....
..........aaaaa
Cells Solved: 300 of 300
Lines in Puzzle: 35
Lines Processed: 1158 (3300%)
Exhaustive Search: 13 cells in 2 passes
Backtracking: 105 probes, 32 guesses, 32 backtracks
Probe Sequences: 32
  Found Contradiction: 31 (0 adj, 31 2-neigh)
  Found Solution:      1 (0 adj, 1 2-neigh)
  Choose Optimum:      0 (0 adj, 0 2-neigh)
Total probes: 105 (0 adj, 105 2-neigh)
Plod cycles: 1, Sprint cycles: 0
Cache Hits: 576/1102 (52.0%) Adds: 494  Flushes: 0
Processing Time: 0.001668 sec 
```

### With `nonogrid`

`nonogrid` can provide a better and more comprehensive visual representation of ambiguities in non-unique nonograms. Sadly, it doesn't tell you anything about the difficulty of a nonogram.

Make sure to install `nonogrid` with `cargo install --features=xml,web nonogrid` to allow parsing the XML format (and to enable directly downloading nonograms from the web, because why not). Then, to evaluate an image, do:

```
convert-nonogram examples/tea.png | nonogrid
```

### With the Olsak solver
The [Olsak solver] comes in a tarball and doesn't even have a makefile! (Just do `gcc grid.c -o grid` to build it.) It accepts a different input format. It does provide some difficulty information, but I haven't yet learned to understand it.

[Olsak solver]:  http://www.olsak.net/grid.html

```
convert-nonogram examples/tea.png --olsak | grid -
```

### With Nonny

[Nonny] is a nonogram editor that can open Olask-formatted nonograms. Its solver is a bit unsophisticated, but you can watch it work, which can give you an idea of what parts of the puzzle are easy to deal with and what parts are trickier. 

[Nonny]: https://github.com/gkikola/nonny

```
convert-nonogram examples/tea.png --olsak > ~/.local/share/nonny/puzzles/tea.g
```

To export a puzzle from Nonny, I, uh, take a screenshot of the thumbnail in the Gimp, crop it, and resize the image with "none" as the interpolation technique. Maybe `convert-nonogram` ought to accept some other nonogram formats as input.