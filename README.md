
# Nonvolatile &emsp; [![Latest Version]][crates.io]

[Latest Version]: https://img.shields.io/crates/v/nonvolatile
[crates.io]: https://crates.io/crates/nonvolatile

**Nonvolatile is a library for storing persistent settings and configuration data out of the way.**

Nonvolatile state is created by instantiating a `State` instance with a name, usually the name of the program creating it.
Any set values are written to disk either in `~/.local/.../[name]` or in `%appdata%/.../[name]`, depending on the platform being used. Values persist until they are overwritten, and can be accessed by any program that loads the state with that name.

**Note:** This library is still a work-in-progress and is not yet fully functional.

## Example

```rust
use nonvolatile::State;
use generic_error::*;

fn main() -> Result<()> {
	
	//create a new state instance with the name "foo"
	let mut state = State::load_else_create("foo")?;
	//set a variable in foo
	state.set("var", "some value")?;
	
	//destroy the state variable
	drop(state);
	
	//create a new state instance
	let state = State::load_else_create("foo")?;
	//retrieve the previously set variable.
	println!("foo: {}", state.get("var").unwrap());  //"some value"	
	Ok(())
}
```
