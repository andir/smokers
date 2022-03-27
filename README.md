# smokers - a Rust version of [smoke](https://github.com/SamirTalwar/smoke)

This is a rust version of the excellent
[smoke](https://github.com/SamirTalwar/smoke) tool. The motivation for
rewriting comes out of the dependency hell that haskell is and the
platform support nightmare that comes with it. It didn't want to
require contributors to compile a test tool for multiple times the
build time of my actual project.

Gaining feature parity isn't a priority or goal. For now this is just
a very basic (inspired) version that allows running various tests and
confirm assumptions.
