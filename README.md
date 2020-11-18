# `convert-nonogram`

`convert-nonogram` is a tool that converts images to nonograms. (Why don't we just use indexed PNGs as the standard interchange format for nonograms?) Currently, it only outputs the widely-used XML-based `webpbn` format, because `pbnsolve` and `nonogrid` both accept that format.

## How to use it

I use `convert-nonogram` to evaluate the solvability of nonograms while editing them as an image. You can install it with `cargo install convert-nonogram`.

All images supported by the [image] crate are supported as input, but if you try to create JPEG nonograms, you're going to have a bad time.

[image]: https://crates.io/crates/image

### With `pbnsolve`
This is what I do, since [`pbnsolve`] provides useful information about difficulty. You'll have to download and install it [from a tarball].

[`pbnsolve`]: https://webpbn.com/pbnsolve.html
[from a tarball]: https://code.google.com/archive/p/pbnsolve/downloads

Then, to evaluate an image, do:

```
convert-nonogram /path/to/image | pbnsolve -tu
```

(`-t` requests detailed difficulty output, and `-u` requires checking for uniqueness. You can add `-b` to suppress output of the solved grid, but it's useful when debugging a non-unique nonogram)

### With `nonogrid`

`nonogrid` can provide a better and more comprehensive visual representation of ambiguities in non-unique nonograms.

Make sure to install `nonogrid` with `cargo install --features=xml,web nonogrid` to allow parsing the XML format (and to enable directly downloading nonograms from the web, because why not). Then, to evaluate an image, do:

```
convert-nonogram /path/to/image | nonogrid
```

### With the Olsak solver
The olsak solver comes in a [tarball] and doesn't even have a makefile! (Just do `gcc grid.c -o grid` to build it.) It accepts a different input format.

```
convert-nonogram /path/to/image --olsak | grid -
```
