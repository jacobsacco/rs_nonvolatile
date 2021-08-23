
/*
nonvolatile

Jacob Sacco
August 2019
*/

//!Nonvolatile is a library for storing persistent settings and configuration data out of the way.
//!
//!Nonvolatile state is created by instantiating a `State` instance with a name, 
//!usually the name of the program creating it. Any set values are written to disk 
//!in some common directory depending on the 
//!platform being used. Values persist until they are overwritten, and can be 
//!accessed by any program that loads the state with that name. `State` instances
//!are exclusive (i.e., two programs or two instances of the same program cannot 
//!have the same `State` open at the same time).
//!
//!Most of the builtin types, and any type that implements `serde::Serialize`/`Deserialize` 
//!may be passed into and read from `State::set` and `State::get`.
//!
//!
//!# Example
//!
//!```rust
//!use nonvolatile::State;
//!use generic_error::*;
//!
//!fn main() -> Result<()> {
//!	
//!	//create a new state instance with the name "foo"
//!	let mut state = State::load_else_create("foo")?;
//!	//set a variable in foo
//!	state.set("var", String::from("some value"))?;
//!	
//!	//destroy the state variable
//!	drop(state);
//!	
//!	//create a new state instance
//!	let state = State::load_else_create("foo")?;
//!	//retrieve the previously set variable.
//!	println!("foo: {}", state.get::<String>("var").unwrap());  //"some value"	
//!	Ok(())
//!}
//!```
//!
//!
//!# Notes
//!
//!By default, state for a given name will be stored in 
//!`$HOME/.local/rust_nonvolatile/<name>` for Linux and MacOS systems, and 
//!`%appdata%\rust_nonvolatile\<name>` for Windows systems. If $HOME or %appdata% 
//!are not defined in the program environment, then nonvolatile will fall back to 
//!`/etc` and `C:\ProgramData` for Linux/MacOS and Windows respectively. 
//!
//!If your environment is unreliable, or you have a location where you'd rather keep settings
//!and configuration, the default storage location can be overridden using the 
//!`*_from` functions (`new_from` instead of `new`, `load_from` instead of `load`, 
//!`load_else_create_from` instead of `load_else_create`). 
//!
//!Be careful to be consistent 
//!with the storage location! If you use a state from one location during one instance
//!of your program, and then use a state from a different location during the next,
//!you will be left with two non-matching states with the same name in different places.
//!
//!
//!# Available State Functions
//!
//!```rust 
//! pub fn set<T>               (&mut self, var: &str, value: T) -> Result<()>
//! pub fn get<'de, T>          (&self, var: &str)               -> Option<T>
//! pub fn has                  (&self, item: &str)              -> bool
//! pub fn delete               (&mut self, name: &str)          -> Result<()>
//!
//! pub fn load_else_create     (name: &str)                     -> Result<State>
//! pub fn load_else_create_from(name: &str, path: &str)         -> Result<State>
//! pub fn new                  (name: &str)                     -> Result<State>
//! pub fn new_from             (name: &str, storage_path: &str) -> Result<State>
//! pub fn load                 (name: &str)                     -> Result<State>
//! pub fn load_from            (name: &str, storage_path: &str) -> Result<State>
//! pub fn destroy_state        (name: &str)
//! pub fn destroy_state_from   (name: &str, storage_path: &str)
//! ```

#![crate_name = "nonvolatile"]
#![crate_type = "lib"]
#![crate_type = "rlib"]

use whoami;
use whoami::Platform::{Linux, Windows, MacOS};
use serde::{Serialize, Deserialize};
use serde_yaml;
use std::fs::{
	copy,
	create_dir_all, 
	rename, 
	metadata,
	read_to_string, 
	OpenOptions,
	remove_file,
	remove_dir_all,
	canonicalize,
};
use std::collections::HashMap;
use std::env;
use std::io::Write;
use std::process;
use std::thread;
use std::time;
use std::mem::drop;
use std::vec::Vec;
use rand::random;
use sysinfo::{System, ProcessExt, SystemExt};
use generic_error::{Result, GenErr, GenericError};
use fs_util::copy_dir;

#[cfg(test)]
mod tests;


#[derive(Serialize, Deserialize, Debug)]
pub struct State {
	name: String,
	path: String,
	identifier: String,
	lockfile_path: String,
	manifest_path: String,
	tmp_manifest_path: String,
	items: HashMap<String, String>,
	preserved: HashMap<String, String>,
}


enum WhoOwns {
	Me,
	Other,
	Nobody,
}


fn build_var_path(var: &str, sub_dir: &str) -> Result<String> {
	let s = match env::var(var) {
		Ok(s) => s,
		Err(_) => match whoami::platform() {	//An error here indicates that either $HOME or %appdata% is not defined
			Windows => String::from("C:/ProgramData"),
			_ => String::from("/etc")
		}
	};
	Ok(format!("{}/{}", s, sub_dir))
}


fn get_storage_dir() -> Result<String> {
	match whoami::platform() {
		Linux => {
			build_var_path("HOME", ".local/rust_nonvolatile")
		},
		Windows => {
			build_var_path("appdata", "rust_nonvolatile")
		},
		MacOS => {
			build_var_path("HOME", ".local/rust_nonvolatile")
		}
		_ => GenErr!("nonvolatile: {} not supported", whoami::platform()),
	}
}


fn get_state_id() -> Result<String> {
	let this_pid = process::id();
	let mut system = System::new();
	system.refresh_processes();
	let this_proc = match system.get_processes().get(&(this_pid as i32)) {
		Some(process) => process,
		None => return GenErr!("nonvolatile internal error: my pid should be {} but no process is listed at that PID", this_pid)
	};
	let exe_path = this_proc.exe().to_string_lossy().to_string();
	Ok(format!("{}\n{}\n{}", process::id(), random::<u32>(), exe_path))
}


fn match_state_id(my_id: &str, read_id: &str) -> WhoOwns {
	if my_id == read_id {
		return WhoOwns::Me;
	}
	let parts: Vec<&str> = read_id.split("\n").collect();
	let parts = match parts.len() {
		3 => (parts[0], parts[1], parts[2]),
		_ => return WhoOwns::Nobody,
	};
	let read_pid: u32 = match parts.0.parse() {
		Ok(pid) => pid,
		Err(_) => return WhoOwns::Nobody,
	};
	
	let mut system = System::new();
	system.refresh_processes();
	
	for (other_pid, process) in system.get_processes() {
		let exe_path = process.exe().to_string_lossy().to_string();
		if *other_pid as u32 == read_pid && parts.2 == &exe_path {
			return WhoOwns::Other;
		}
	}
	WhoOwns::Nobody
}


fn get_lock_acquired(lockfile_path: &str, state_id: &str) -> Result<bool> {
	let mdata = match metadata(lockfile_path) {
		Ok(mdata) => mdata,
		Err(_) => return Ok(false),
	};
	if !mdata.is_file() {
		return Ok(false);
	}
	let read_id = read_to_string(lockfile_path)?;
	match match_state_id(state_id, &read_id) {
		WhoOwns::Me => return Ok(true),
		WhoOwns::Other => return GenErr!("lockfile {} already owned by state {}", lockfile_path, read_id),
		WhoOwns::Nobody => return Ok(false),
	}
}


fn acquire_dir(lockfile_path: &str, state_id: &str) -> Result<()> {
	match get_lock_acquired(lockfile_path, state_id) {
		Ok(true) => return Ok(()),
		Ok(false) => (),
		Err(e) => {
			return Err(e)
		},
	};
	
	let _ = remove_file(lockfile_path);
	let mut file = OpenOptions::new().write(true).create(true).open(lockfile_path)?;
	match write!(file, "{}", state_id) {
		Ok(_) => (),
		Err(e) => {
			let _ = remove_file(lockfile_path);
			return Err(e.into());
		},
	};
	drop(file);
	
	thread::sleep(time::Duration::new(0, 1000));
	match get_lock_acquired(lockfile_path, state_id) {
		Ok(true) => Ok(()),
		Ok(false) => GenErr!("Nobody owns the lock, but I still managed to fail to acquire it!"),
		Err(e) => Err(e),
	}
}


impl State {

	fn write_manifest(&self) -> Result<()> {
		let mut file = OpenOptions::new().write(true).create(true).open(&self.tmp_manifest_path)?;
		let data = serde_yaml::to_vec(self)?;
		file.write(&data)?;
		rename(&self.tmp_manifest_path, &self.manifest_path)?;
		Ok(())
	}
	
	
	///Set a variable with name `var` and value `value`. 
	///
	///The name of the set value must be distinct from any other values you set,
	///but otherwise no restrictions apply. The type of `value` must be serializable, 
	///but no other restrictions apply. 
	///
	///The value is written out to storage immediately.
	///
	///### Example
	///
	///```rust
	///let my_var = String::from("this is like a string or something");
	///state.set("my var", my_var);
	///
	///let some_other_var: HashMap<u64, String> = HashMap::new();
	///... //add some stuff to the map
	///state.set("some_other_var", some_other_var.clone()) //save the map for later!
	///```
	pub fn set<T>(&mut self, var: &str, value: T) -> Result<()> where T: Serialize {
		if self.preserved.contains_key(var) {
			return GenErr!("nonvolatile: can't set a variable with the same name as a preserved file/folder");
		}
		let _ = self.items.insert(String::from(var), serde_yaml::to_string(&value)?);
		self.write_manifest()
	}
	

	///Try to retrieve a variable that was previously written to storage. 
	///
	///The return will be the value if it can be found, or None if:
	/// * no value with that name is stored, or
	/// * the stored value had a type incompatible with the `get` call.
	///
	///`get` does not modify or remove the stored value, it only reads it.
	///
	///### Example
	///
	///```rust
	///let my_var: String = state.get("my var");
	///
	///let some_other_var = state.get::<HashMap<u64, String>>("some_other_var");
	///```
	pub fn get<'de, T>(&self, var: &str) -> Option<T> where for<'a> T: Deserialize<'a> {
		let item = self.items.get(var)?;
		match serde_yaml::from_str(item) {
			Ok(obj) => Some(obj),
			Err(_) => None,
		}
	}


	///Check if the given item/key exists in the state.
	///
	///### Example
	///
	///```rust
	///state.delete("user_wants_to_die");
	///println!("{}", state.has("user_wants_to_die")); // false
	///state.set("user_wants_to_die", true);
	///println!("{}", state.has("user_wants_to_die")); // true
	///```
	pub fn has(&self, item: &str) -> bool {
		self.items.contains_key(item) || self.preserved.contains_key(item)
	}
	
	
	///Delete a stored variable. If the variable does not exist, nothing happens.
	///
	///### Example
	///
	///```rust
	///let my_var = String::from("wait no don't delete me");
	///state.set("my var", my_var);
	///...
	/// // oop, looks like we don't need my_var to be stored for some reason
	///state.delete("my var");
	///```
	pub fn delete(&mut self, name: &str) -> Result<()> {
		let _ = self.items.remove(name);
		if let Some(_) = self.preserved.remove(name) {
			let path = format!("{}/{}", &self.path, name);
			remove_file(&path)?;
			remove_dir_all(&path)?;
		}
		self.write_manifest()
	}


	///Load state of the given name if it exists. If not, create new state and return that.
	///
	///The name must obey naming rules for your filesystem, so spaces and special
	///characters should be avoided.
	///
	///### Example
	///
	///```rust
	///let state = State::load_else_create("my_state");
	///let my_var = String::from("this is like a string or something");
	///state.set("my var", &my_var);
	///```
	pub fn load_else_create(name: &str) -> Result<State> {
		State::load(name).or_else(|_| State::new(name))
	}
	
	
	///Load state of the given name from the given custom storage location if the 
	///state exists exists. If not, create new state at the custom location and 
	///return that.
	///
	///The name must obey naming rules for your filesystem, so spaces and special
	///characters should be avoided.
	///
	///the storage path may be relative or absolute, and doesn't have to already exist 
	///(but it must be creatable). The state will be stored in 
	///`<storage_path>/rust_nonvolatile`. Accessing that location directly is not recommended.
	///
	///### Example
	///
	///```rust
	///let state = State::load_else_create_from("my_state", ".");	// load or create state from the CWD
	///let my_var = String::from("this is like a string or something");
	///state.set("my var", &my_var);
	///```
	pub fn load_else_create_from(name: &str, path: &str) -> Result<State> {
		State::load_from(name, path).or_else(|_| State::new_from(name, path))
	}


	///Create a new State object with the given name.
	///
	///The name must obey naming rules for your filesystem, so spaces and special
	///characters should be avoided.
	///
	///If there is a preexisting state with that name, it will be overwritten by `new`.
	///If the preexisting state is open by someone or something else, then `new` will fail
	///and return an error.
	///
	///### Example
	///
	///```rust
	///let state = State::new("my_state");
	///let my_var = String::from("this is like a string or something");
	///state.set("my var", my_var);
	///```
	pub fn new(name: &str) -> Result<State> {
		let dir = get_storage_dir()?;
		State::new_from(name, &dir)
	}
	

	///Create a new State object with the given name, and a custom storage location.
	///
	///The name must obey naming rules for your filesystem, so spaces and special
	///characters should be avoided.
	///
	///the storage path may be relative or absolute, and doesn't have to already exist 
	///(but it must be creatable). The state will be stored in 
	///`<storage_path>/rust_nonvolatile`. Accessing that location directly is not recommended.
	///
	///If there is a preexisting state with that name, it will be overwritten by `new_from`.
	///If the preexisting state is open by someone or something else, then `new_from` will fail
	///and return an error.
	///
	///### Example
	///
	///```rust
	///let state = State::new_from("my_state", ".");	// create the state in the CWD
	///let my_var = String::from("this is like a string or something");
	///state.set("my var", my_var);
	///```
	pub fn new_from(name: &str, storage_path: &str) -> Result<State> {
		let path = format!("{}/{}", storage_path, name);
		create_dir_all(&path)?;
		
		let items: HashMap<String, String> = HashMap::new();
		let preserved: HashMap<String, String> = HashMap::new();
		
		let state_id = match get_state_id() {
			Ok(id) => id,
			Err(e) => return Err(e.into())
		};
		let lockfile_path = format!("{}/{}", &path, "~rust_nonvolatile.lock");
		acquire_dir(&lockfile_path, &state_id)?;
		
		let state = State {
			name: String::from(name),
			path: path.clone(),
			identifier: state_id,
			lockfile_path: lockfile_path.clone(),
			manifest_path: format!("{}/{}", &path, ".manifest"),
			tmp_manifest_path: format!("{}/{}", &path, ".manifest_tmp"),
			items: items,
			preserved: preserved,
		};
		
		match state.write_manifest() {
			Ok(_) => Ok(state),
			Err(e) => {
				let _ = remove_file(&lockfile_path);
				Err(e.into())
			}
		}
	}


	///Attempt to load state of the given name
	///
	///If there is no state with that name, an error will be returned.
	///
	///### Example
	///
	///```rust
	///let state = State::load("my_state");
	///let my_var = String::from("this is like a string or something");
	///state.set("my var", my_var);
	///```
	pub fn load(name: &str) -> Result<State> {
		let dir = get_storage_dir()?;		
		State::load_from(name, &dir)
	}
	
	
	///Attempt to load state of the given name from a custom storage location.
	///
	///If there is no state with that name at that location, an error will be returned.
	///
	///### Example
	///
	///```rust
	///let state = State::load_from("my_state", ".");	// load state from the CWD
	///let my_var = String::from("this is like a string or something");
	///state.set("my var", &my_var);
	///```
	pub fn load_from(name: &str, storage_path: &str) -> Result<State> {
		let path = format!("{}/{}", storage_path, name);
		let manifest_path = format!("{}/{}", &path, ".manifest");
		
		let state_id = match get_state_id() {
			Ok(id) => id,
			Err(e) => return Err(e.into())
		};
		let lockfile_path = format!("{}/{}", &path, "~rust_nonvolatile.lock");
		
		acquire_dir(&lockfile_path, &state_id)?;
		
		let data = match read_to_string(&manifest_path) {
			Ok(data) => data,
			Err(e) => {
				let _ = remove_file(&lockfile_path);
				return Err(GenericError::from(e));
			}
		};
		
		let mut state: State = match serde_yaml::from_str(&data) {
			Ok(state) => state,
			Err(e) => {
				let _ = remove_file(&lockfile_path);
				return Err(GenericError::from(e));
			}
		};
		
		state.identifier = state_id;
		state.lockfile_path = lockfile_path;
		
		Ok(state)
	}
	
	
	///Destroy the state of the given name. If no state exists with that name, nothing happens.
	///
	///### Example
	///
	///```rust
	///let state = State::load_else_create("foo").unwrap();
	///... // do stuff with the state
	///drop(state);
	///
	/// // ... 
	///
	/// //oh, turns out we don't need those settings stored after all...?
	///State::destroy_state("foo");
	///```
	pub fn destroy_state(name: &str) {
		if let Ok(dir) = get_storage_dir() {
			let path = format!("{}/{}", dir, name);
			let _ = remove_dir_all(path);
		} 
	}


	///Destroy the state of the given name at the given custom storage location. 
	///If no state exists with that name at that location, nothing happens.
	///
	///### Example
	///
	///```rust
	///let state = State::load_else_create_from("foo", ".").unwrap(); // load or create the state in the CWD
	///... // do stuff with the state
	///drop(state);
	///
	/// // ... 
	///
	/// //oh, turns out we don't need those settings stored after all...?
	///State::destroy_state_from("foo", ".");
	///```
	pub fn destroy_state_from(name: &str, storage_path: &str) {
		let path = format!("{}/{}", storage_path, name);
		let _ = remove_dir_all(path);
	}
	
	
	fn _preserve(&mut self, path: &str, name: &str) -> Result<()> {
		if self.items.contains_key(name) {
			return GenErr!("nonvolatile: can't preserve a file with the same name as a set variable");
		}
		let path = match canonicalize(path)?.to_str() {
			Some(p) => String::from(p),
			None => return GenErr!("nonvolatile preserve: failed to canonicalize path"),
		};
		let tmp_name = format!("tmp_{}", name);
		let tmp_dest = format!("{}/{}", &self.path, &tmp_name);
		let dest = format!("{}/{}", &self.path, name);
		if metadata(&path)?.is_dir() {
			copy_dir(&path, &tmp_dest)?;
		}
		else {
			copy(&path, &tmp_dest)?;
		}
		
		let _ = self.preserved.insert(String::from(name), String::from(path));
		self.write_manifest()?;
		
		rename(tmp_dest, dest)?;
		Ok(())
	}


	fn _restore(&self, name: &str) -> Result<()> {
		let path = match self.preserved.get(name) {
			Some(p) => p,
			None => return GenErr!("Nothing by the name '{}' has been preserved", name),
		};
		self._restore_to(name, path)
	}
	
	
	fn _restore_to(&self, name: &str, path: &str) -> Result<()> {
		if !self.preserved.contains_key(name) {
			return GenErr!("Nothing by the name '{}' has been preserved", name);
		}
		let preserved_path = format!("{}/{}", &self.path, name);
		if metadata(&preserved_path)?.is_dir() {
			copy_dir(&preserved_path, path)?;
		}
		else {
			copy(&preserved_path, path)?;
		}
		Ok(())
	}
}


impl Drop for State {
	fn drop(&mut self) {
		let _ = remove_file(&self.lockfile_path);
	}
}
