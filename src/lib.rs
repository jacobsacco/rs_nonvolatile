
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
	let this_proc = match system.processes().get(&(this_pid as i32)) {
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
	
	for (other_pid, process) in system.processes() {
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
	
	
	pub fn set<T>(&mut self, var: &str, value: T) -> Result<()> where T: Serialize {
		if self.preserved.contains_key(var) {
			return GenErr!("nonvolatile: can't set a variable with the same name as a preserved file/folder");
		}
		let _ = self.items.insert(String::from(var), serde_yaml::to_string(&value)?);
		self.write_manifest()
	}
	
	
	pub fn get<'de, T>(&self, var: &str) -> Option<T> where for<'a> T: Deserialize<'a> {
		let item = self.items.get(var)?;
		match serde_yaml::from_str(item) {
			Ok(obj) => Some(obj),
			Err(_) => None,
		}
	}
	
	
	pub fn delete(&mut self, name: &str) -> Result<()> {
		let _ = self.items.remove(name);
		if let Some(_) = self.preserved.remove(name) {
			let path = format!("{}/{}", &self.path, name);
			remove_file(&path)?;
			remove_dir_all(&path)?;
		}
		self.write_manifest()
	}
	
	
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
	
	
	pub fn new(name: &str) -> Result<State> {
		let dir = get_storage_dir()?;
		State::new_from(name, &dir)
	}
	
	
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
	
	
	pub fn load(name: &str) -> Result<State> {
		let dir = get_storage_dir()?;		
		State::load_from(name, &dir)
	}
	
	
	pub fn load_else_create(name: &str) -> Result<State> {
		State::load(name).or_else(|_| State::new(name))
	}
	
	
	pub fn load_else_create_from(name: &str, path: &str) -> Result<State> {
		State::load_from(name, path).or_else(|_| State::new_from(name, path))
	}
	
	
	pub fn destroy_state(name: &str) {
		if let Ok(dir) = get_storage_dir() {
			let path = format!("{}/{}", dir, name);
			let _ = remove_dir_all(path);
		} 
	}
    
    
    pub fn destroy_state_from(name: &str, storage_path: &str) {
        let path = format!("{}/{}", storage_path, name);
        let _ = remove_dir_all(path);
    }
	
	
	pub fn has(&self, item: &str) -> bool {
		self.items.contains_key(item) || self.preserved.contains_key(item)
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
