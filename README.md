# What?

This is a program useful for running X11 applications on a *local* desktop from within an SSH session *connected to* that desktop.

# Where?

It's meant for a Linux host, but it should run on any UNIX-compatible system with a Plan 9 style `/proc` directory that contains processes that contain an `environ` file with zero-terminated key-value pairs in it. (Obviously, the computer running the SSH client can be using any operating system.)

# Why?

If you have to ask the question, you probably don't need this. It's not a thing that most people should need.

# How?

[Install Rust](https://rust-lang.org/learn/get-started/). then run:

```sh
git clone https://github.com/SolraBizna/x11hunter && cargo install --path=x11hunter`
```

Now that `x11hunter` is installed, you can run a program like this:

```sh
env `x11hunter` my_program
```

Or set the variables for a whole shell session like this:

```sh
export `x11hunter`
```

# No, I mean, how does it work?

`x11hunter` looks at a representative sample of your processes, sniffing for values of the `DISPLAY` and `XAUTHORITY` environment variables. It will then output the most popular one it finds.

# Legalese

`x11hunter` is copyright 2026, Solra Bizna, and licensed under either of:

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or
   <http://www.apache.org/licenses/LICENSE-2.0>)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the `x11hunter` module by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

All that legalese aside, this is a pretty trivial project. I've only slapped this dual license on here because it's my standard go-to.