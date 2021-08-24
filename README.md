
# Nonvolatile &emsp; [![Latest Version]][crates.io]

[Latest Version]: https://img.shields.io/crates/v/nonvolatile
[crates.io]: https://crates.io/crates/nonvolatile

**Nonvolatile is a library for storing persistent settings and configuration data out of the way.**

Nonvolatile state is created by instantiating a `State` instance with a name, 
usually the name of the program creating it. Any set values are written to disk 
in some common directory depending on the 
platform being used. Values persist until they are overwritten, and can be 
accessed by any program that loads the state with that name. `State` instances
are exclusive (i.e., two programs or two instances of the same program cannot 
have the same `State` open at the same time).

Most of the builtin types, and any type that implements `serde::Serialize`/`Deserialize` 
may be passed into and read from `State::set` and `State::get`.

[Check out the documentation here for more detailed information](https://docs.rs/nonvolatile)


# Example

```rust
use nonvolatile::State;
use generic_error::*;

fn main() -> Result<()> {
	
	//create a new state instance with the name "foo"
	let mut state = State::load_else_create("foo")?;
	//set some variables in foo
	state.set("var", "some value")?;
	state.set("user_wants_pie", true)?;
	
	//destroy the state variable
	drop(state);
	
	//create a new state instance
	let state = State::load_else_create("foo")?;
	//retrieve the previously set variable.
	assert_eq!(state.get::<bool>("user_wants_pie"), Some(true));
	assert_eq!(state.get::<String>("var").unwrap(), "some value");
	Ok(())
}
```


# Notes

By default, state for a given name will be stored in 
`$HOME/.local/rust_nonvolatile/<name>` for Linux and MacOS systems, and 
`%appdata%\rust_nonvolatile\<name>` for Windows systems. If $HOME or %appdata% 
are not defined in the program environment, then nonvolatile will fall back to 
`/etc` and `C:\ProgramData` for Linux/MacOS and Windows respectively. 

If your environment is unreliable, or you have a location where you'd rather keep settings
and configuration, the default storage location can be overridden using the 
`*_from` functions (`new_from` instead of `new`, `load_from` instead of `load`, 
`load_else_create_from` instead of `load_else_create`). 

Be careful to be consistent 
with the storage location! If you use a state from one location during one instance
of your program, and then use a state from a different location during the next,
you will be left with two non-matching states with the same name in different places.


# Available State Functions

```rust 
 pub fn set<T>               (&mut self, var: &str, value: T) -> Result<()>
 pub fn get<'de, T>          (&self, var: &str)               -> Option<T>
 pub fn has                  (&self, item: &str)              -> bool
 pub fn delete               (&mut self, name: &str)          -> Result<()>

 pub fn load_else_create     (name: &str)                     -> Result<State>
 pub fn load_else_create_from(name: &str, storage_path: &str) -> Result<State>
 pub fn new                  (name: &str)                     -> Result<State>
 pub fn new_from             (name: &str, storage_path: &str) -> Result<State>
 pub fn load                 (name: &str)                     -> Result<State>
 pub fn load_from            (name: &str, storage_path: &str) -> Result<State>
 pub fn destroy_state        (name: &str)
 pub fn destroy_state_from   (name: &str, storage_path: &str)
 ```
