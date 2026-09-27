Re:Zoids Saga
=============

A free, native port of Zoids Saga (Game Boy Advance, Japan). This package
holds only the program: no part of the game comes with it. You need your
own copy of the Zoids Saga ROM (Japan, Rev 1), dumped from your cartridge.

This demo plays the first three chapters, from the title to the party's
return from the Mount Ossa fortress.


Starting
--------

macOS    Drag "Re Zoids Saga" to Applications and open it. The app is not
         signed by an identified developer, so the first time macOS
         refuses it: right-click (or Control-click) the app, choose Open,
         then Open again. On recent versions, open System Settings >
         Privacy & Security and choose "Open Anyway".

Windows  Unzip the folder anywhere and run re-zoids-saga.exe. The program
         is not signed, so Windows may show "Windows protected your PC":
         choose "More info", then "Run anyway".

Linux    Unpack the folder anywhere and run ./re-zoids-saga. It needs a
         desktop with X11 or Wayland; the file dialog uses the desktop's
         portal or zenity.


The launcher
------------

Choose your ROM and, if you like, a translation (.po file), then Play.
Options holds the window's size, fullscreen, the filter, the volume, and
the keys and gamepad buttons. Your choices are remembered.

Default keys:  arrows move, X = A, Z = B, Return = START,
               Backspace = SELECT, A = L, S = R.
Gamepads:      the D-pad or left stick moves, the right face button is A,
               the bottom one B, Start and Back are START and SELECT, the
               shoulders L and R.
Esc asks whether to quit.

Saves are kept next to the ROM, in the original's format (.sav), so they
also work in emulators and on the cartridge.


License
-------

Re:Zoids Saga is free software under the GNU General Public License,
version 3 (LICENSE.txt). Its source code:
https://github.com/serivt/re-zoids-saga

Zoids Saga and its content belong to their respective owners. This
project is not affiliated with or endorsed by them.
