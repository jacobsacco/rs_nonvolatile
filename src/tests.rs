
/*
tests for nonvolatile

Jacob Sacco
August 2019
*/

use super::*;
use std::sync::Mutex;
use std::fs::remove_dir_all;
use lazy_static::lazy_static;


fn destroy_state(name: &str) {
	println!("Destroying any pre-existing state for {}", name);
	State::destroy_state(name);
	println!("Done");
}


fn test_state(s: &mut State) {
	println!("Testing if state object is functional");
	let _ = s.delete("test_var");
	assert_eq!(s.has("test_var"), false);
	s.set("test_var", "foo".to_string()).unwrap();
	assert_eq!(s.has("test_var"), true);
	assert_eq!(s.get("test_var"), Some(String::from("foo")));
	println!("Done");
}

lazy_static! {
	static ref TEST_ID: Mutex<u32> = Mutex::new(0);
}


fn setup_env() -> String {
	let mut id = TEST_ID.lock().unwrap();
	*id += 1;
	let name = format!("test{}", id);
	destroy_state(&name);
	name
}


#[test]
#[should_panic]
fn test_destroy_state() {
	let name = setup_env();
	let s = match State::load_else_create(&name) {
		Ok(s) => s,
		Err(e) => {
			println!("load_else_create failed! {:?}", e);
			return;
		}
	};
	
	drop(s);
	
	destroy_state(&name);
	
	let _ = State::load(&name).unwrap();
}


#[test]
#[should_panic]
fn test_dir_locking() {
	let name = setup_env();
	let mut state = match State::load_else_create(&name) {
		Ok(state) => state,
		Err(_e) => return,
	};
	if let Err(_e) = state.set("var", String::from("some value")) {
		return;
	}
	
	let state2 = State::load_else_create(&name).unwrap();
	println!("foo: {:?}", state2.get::<String>("var"));  //should never reach this point
}


#[test]
#[should_panic]
fn test_name_abuse1() {
	let _ = State::load_else_create("\\\\?\\> '.|:..&\\/().,;?* \" 🙂 lol").unwrap();
}


#[test]
#[should_panic]
fn test_name_abuse2() {
	let _ = State::load_else_create("..").unwrap();
}


#[test]
#[should_panic]
fn test_name_abuse3() {
	let _ = State::load_else_create("../should_not_exist").unwrap();
}


#[test]
fn test_custom_storage_dir() {
	let name = setup_env();
	let custom_dir = "./~rust_nonvolatile_test_tmp_dir";
	State::destroy_state_from(&name, custom_dir);
	State::destroy_state(&name);
	let mut s = State::new_from(&name, custom_dir).unwrap();
	test_state(&mut s);
	s.set("check persistence", true).unwrap();
	drop(s);

	match State::load(&name) {
		Ok(_) => panic!("Successfully loaded a state that should not exist"),
		Err(_) => ()
	};

	let mut s = State::load_from(&name, custom_dir).unwrap();
	test_state(&mut s);
	assert_eq!(s.get("check persistence"), Some(true));
	drop(s);

	remove_dir_all("./~rust_nonvolatile_test_tmp_dir").unwrap();
}


#[test]
fn test_new_state() {
	let name = setup_env();
	
	let mut s = State::new(&name).unwrap();
	test_state(&mut s);
}


#[test]
fn test_load_state() {
	let name = setup_env();
	{
		let _ = State::new(&name).unwrap();
	}
	let mut s = State::load(&name).unwrap();
	test_state(&mut s);
}

#[test]
fn test_load_else_create() {
	let name = setup_env();
	
	let mut s = State::load_else_create(&name).unwrap();
	test_state(&mut s);
}

#[test]
fn test_manifest_write() {
	let name = setup_env();
	{
		let mut s = State::load_else_create(&name).unwrap();
		s.set("test manifest write", String::from("foobar")).unwrap();
	}
	
	{
		let mut s = State::load_else_create(&name).unwrap();
		assert_eq!(s.get("test manifest write"), Some(String::from("foobar")));
		s.delete("test manifest write").unwrap();
	}
	
	{
		let s = State::load_else_create(&name).unwrap();
		assert_eq!(s.has("test manifest write"), false);
	}
}

#[test]
fn test_has() {
	let name = setup_env();
	let mut s = State::load_else_create(&name).unwrap();
	assert_eq!(s.has("something"), false);
	s.set("something", String::from("something")).unwrap();
	assert_eq!(s.has("something"), true);
	assert_eq!(s.has("something else"), false);
}

#[test]
fn test_delete_var() {
	let name = setup_env();
	let mut s = State::load_else_create(&name).unwrap();
	s.set("foo", String::from("bar")).unwrap();
	assert_eq!(s.has("foo"), true);
	s.delete("foo").unwrap();
	assert_eq!(s.has("foo"), false);
}


#[test]
fn test_example() {
	
	//create a new state instance with the name "foo"
	let mut state = State::load_else_create("foo").unwrap();
	//set some variables in foo
	state.set("var", "some value").unwrap();
	state.set("user_wants_pie", true).unwrap();
	
	//destroy the state variable
	drop(state);
	
	//create a new state instance
	let state = State::load_else_create("foo").unwrap();
	//retrieve the previously set variable.
	assert_eq!(state.get::<bool>("user_wants_pie"), Some(true));
	assert_eq!(state.get::<String>("var").unwrap(), "some value");
}

