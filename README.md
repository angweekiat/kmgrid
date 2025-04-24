# KMGrid

Use your keyboard and this grid interface to replace your mouse!

![Sample](./resources/sample.gif)

KMGrid is a toy project I started to learn Rust, and also to remove the need for a mouse, after switching to using [Kit46](https://github.com/angweekiat/zmk-config-kit46) split keyboard.

KMGrid usage is split into 3 steps:
- On the screen-wide grid, press the particular key to get a smaller grid of the specific region
- On the smaller region grid, press the particular key (right handed keys) to snap the cursor to the particular cell
- In the cell display, you can:
    - left-click / right-click / middle-click
    - use configured controls to move the cursor around
    - scroll up / down 

The repo is currently lacking a lot of functionalities due to time constraints :(

## Prerequitise system libraries:
- libx11-dev
- libxdo-dev

## Build step
```
cargo build
```

## TODO List
- Handle wayland protocol
- Handle x11 protocol instead of using wrapper libs
- Try to detect and exit upon alt-tab; User isn't interested in using app anymore
- Per display colors
- Avoid hardcoded 4x4 grid setup, make it dynamic based on user config

## Bugs
- Doesn't work on wayland
