# `number-loom`

`number-loom` is a powerful tool for constructing puzzles variously known as "Nonograms", "Paint By Numbers", "Griddlers" (and many other names).

![Screenshot of a GUI editor](screenshot.png)

It's still pretty janky, but it's also the most powerful such tool I know of. In particular, it can offer suggestions for how to make an unsolveable puzzle solveable!

## Features

* Supports the following file formats:
  * `webpbn`'s XML-based format (extension: `.xml` or `.pbn`)
  * The format used by the Olsak solver (extension: `.g`)
  * Images (typical extension: `.png`)
  * `char-grid`, a plaintext grid of characters, which it attempts to infer a reasonable character-to-color mapping (extension: `.txt`)
  * HTML, for export only, as a printable puzzle (extension `.html`)
* Has (poorly-tested) support for "Trianograms", a rare variation with triangular cells that may appear as caps to clues.
* A line-logic solver (currently not quite as powerful as it should be!) that provides some difficulty information.
* A tool that searches for one-cell edits that make puzzles closer to solveable.


## Installation and usage

If you don't have `cargo` on your computer already, [install it through `rustup`](https://doc.rust-lang.org/cargo/getting-started/installation.html).

Then run `cargo install number-loom`.

To open the gui, you have to pick a nonogram to open: `number-loom` or `number-loom examples/png/keys.png --gui`.

To solve a puzzle from the command line, do `number-loom examples/png/hair_dryer.png`.

To convert a puzzle from the command line, do `number-loom examples/png/hair_dryer.png /tmp/hair_dryer.xml`.

## Solver

The `number-loom` solver is a line-logic solver only. Algorithmically, it borrows a lot from `pbnsolve`, a powerful and fast nonogram solver, but `number-loom`'s heuristics for processing lines are intended to mimic human solver behavior rather than to maximize speed. (It also currently isn't as powerful as `pbnsolve`'s line-solver in `-aE` mode; this should be fixed!)

It has two modes:
  * "skim", which shoves all clues in a line as far as possible to one side and then the other, and checks to see if any of the clues overlap themselves between the two positions
  * "scrub", which tries each color for each cell in a line, ruling out any colors that cause a contradiction

It stores progress by noting each possibly-remaining color in each cell. Even though a human solver typically only notes down known cells, in my experience this corresponds pretty well to the sort of ad-hoc logic that solvers perform on color nonograms when they glance at the both lines that contain a cell.

## GUI

The GUI is very basic, but you can

* Save and load
* Zoom in and out
* Adjust the size of the canvas (choosing which direction to add or remove lines)
* Add, remove, or recolor palette entries
* Solve the puzzle (it paints gray dots over unsolved cells), optionally automatically after each edit
* Disambiguate

### Disambiguation

This may take a little bit of time, but it's typically reasonably fast for puzzles under 30x30. Cells will be painted with an alternate color, with an opacity proportional to the number of unsolved cells that are resolved if painted that color.

It operates by plain-old brute force. I think there is some potentially-useful less-precise information that could be generated faster, but I think this is more useful.

## Usage with other solvers

### `pbnsolve`

You'll have to download and install `pbnsolve` [from a tarball] (and probably edit the `Makefile` to help it find `libxml2` -- under Ubuntu, you'll need to do `sudo apt install libxml2-dev`). Then (assuming it's on your `$PATH`):

[`pbnsolve`]: https://webpbn.com/pbnsolve.html
[from a tarball]: https://code.google.com/archive/p/pbnsolve/downloads

```
number-loom examples/png/stroller.png - --output-format webpbn | pbnsolve -tu
```

It gives some difficulty information. I believe that "Lines Processed" very roughly corresponds to `number-loom`'s measurement of skims and scrubs (summed together). But `pbnsolve` is currently more powerful for difficult puzzles. For example, it can solve the stroller puzzle, even if you use `-aE` to restrict it to line logic.

### `nonogrid`

`nonogrid`, written in Rust, can provide a nice visual representation of ambiguities on the command line and is capable of non-line-logic solving.

Make sure to install `nonogrid` with `cargo install --features=xml,web,sat nonogrid` to allow parsing the XML-based webpbn format (and to enable directly downloading nonograms from the web, and the SAT-based solver, because why not). Then, to evaluate an image, do:

```
number-loom examples/png/stroller.png - --output-format webpbn | nonogrid
```

### The Olsak solver
The [Olsak solver] comes in a tarball and doesn't even have a makefile! (Just do `gcc grid.c -o grid` to build it.) It accepts a different input format. It does provide some difficulty information, but I haven't yet learned to understand it.

[Olsak solver]:  http://www.olsak.net/grid.html

```
number-loom examples/png/stroller.png - --output-format olsak | grid -
```