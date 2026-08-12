---
id: tutorial_first_build
title: 'Tutorial: Your First Bessemer Application'
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


import { FbInternalOnly, OssOnly, isInternal } from
'docusaurus-plugin-internaldocs-fb/internal';

Welcome! This tutorial will help you learn the basics of building your first
Rust application with Bessemer. We will start from the very beginning, guiding you
step-by-step to get a "Hello, World!" message displayed in your terminal.
Through this practical exercise, you'll acquire foundational skills for working
with Bessemer.

## What We'll Do:

1. Set up a simple project directory.
2. Write a "Hello, World!" program in Rust.
3. Create a BUILD.bsmr file to tell Bessemer how to build our program.
4. Build the Rust program using Bessemer.
5. Run our compiled program using Bessemer.
6. Briefly inspect the build targets we created.

## Prerequisites:

For this tutorial, we will use Rust. You don't need to know how to write Rust.
We will only use a few basics of Rust.

<OssOnly>
- [Follow the previous section to set up Bessemer](../install)
- [Set up Rust](https://rustup.rs/)
</OssOnly>
<FbInternalOnly>
- Set up an [`fbsource`](https://www.internalfb.com/wiki/Repositories/fbsource/#cloning) repo, or [reserve an OnDemand](https://www.internalfb.com/intern/ondemand/) with `fbsource`
</FbInternalOnly>

## Step 1: Create a New Project

<FbInternalOnly>

First, we need a place for our project files. We will put it in
`fbcode/scripts/$USER/bsmr_lab`

1. Create a new directory. In fbsource root

```bash
mkdir -p fbcode/scripts/$USER/bsmr_lab
cd fbcode/scripts/$USER/bsmr_lab
```

</FbInternalOnly>

<OssOnly>

First, we need to create a new bsmr project directory and set up a folder to
put all our files in.

1. Create a new bsmr project directory and create a new directory named
   `bsmr_lab` inside it.

```bash
bsmr init hello_world

mkdir hello_world/bsmr_lab

cd hello_world/bsmr_lab
```

</OssOnly>

2. Inside `bsmr_lab`, create another directory named `greeter_bin`, this is
   where all files of our Bessemer binary package will be stored.

```bash
mkdir greeter_bin
```

3. Create `greeter_bin/src` directory, this is where all source files of our
   binary will live.

```bash
mkdir greeter_bin/src
```

Our project structure should look like this:

<FbInternalOnly>

```
fbcode/scripts/$USER/bsmr_lab
└── greeter_bin
    └── src
```

</FbInternalOnly>

<OssOnly>

```
bsmr_lab
└── greeter_bin
    └── src
```

</OssOnly>

## Step 2: Writing the "Hello, World!" Program

Now, let's write our simple Rust program.

1. Inside the src directory, create a new file named main.rs.
2. Open src/main.rs in your favorite text editor and add the following code:

```rust
fn main() {
    println!("Hello world!");
}
```

This is the main function of our program. It prints "Hello world!" to the
console. But we are not done yet, we need to tell Bessemer how to build our
program.

## Step 3: Defining the Build Target in a BUILD.bsmr File

Next, we need to tell Bessemer about our program and how to build it. We do this
using a `BUILD.bsmr` file.

1. In the root of your `greeter_bin` directory (not inside src), create a new
   file named `BUILD.bsmr`.

2. Add the following content to this BUILD.bsmr file:

<FbInternalOnly>

```python
load("@bsmr_build//rules:rust.bzl", "rust_binary")

rust_binary(
    name = "main",
    srcs = ["src/main.rs"],
)
```

</FbInternalOnly>

<OssOnly>

```python
rust_binary(
    name = "main",
    srcs = ["src/main.rs"],
)
```

</OssOnly>

Let's briefly see what this does (we'll keep explanations minimal, just enough
for this step! ):

<FbInternalOnly>
- `load(...)` is a load statement. It tells Bessemer to load the definition of the
  `rust_binary`.
</FbInternalOnly>
- `rust_binary` is a Bessemer rule that tells Bessemer how to build a Rust binary.
- `name = "main"`: We're giving our build target a name, "main". This is how
  we'll refer to it in Bessemer commands.
- `srcs = ["src/main.rs"]`: This tells Bessemer that the source code for this
  "main" target is our `src/main.rs` file.

Our project structure should look like this:

```
bsmr_lab
└── greeter_bin
    ├── BUILD.bsmr
    └── src
        └── main.rs
```

## Step 4: Building the Application with Bessemer

With our Rust code and BUILD.bsmr file in place, let's build the application!

<FbInternalOnly>

1. Open your terminal and make sure you are in the fbsource root directory.
2. Run the following command:

```bash
bsmr build upstream//scripts/$USER/bsmr_lab/greeter_bin:main --show-output
```

</FbInternalOnly>

<OssOnly>

1. Open your terminal and make sure you are in the `greeter_bin` directory.
2. Run the following command:

```bash
bsmr build :main --show-output
```

</OssOnly>

- `bsmr build :main` tells Bessemer to build the target named main. The `:main`
  part means the target is defined in the BUILD.bsmr file in the root of this package
  (`greeter_bin`).
- `--show-output` tells Bessemer to show the path of our built binary.

3. Expected Output: You should see output similar to this

<FbInternalOnly>

```
...
BUILD SUCCEEDED
upstream//scripts/$USER/bsmr_lab/greeter_bin:main buck-out/v2/gen/fbcode/c32808b9d4f0fdd0/scripts/$USER/bsmr_lab/greeter_bin/__main__/main
```

</FbInternalOnly>

<OssOnly>

```
...
BUILD SUCCEEDED
root//bsmr_lab/greeter_bin:main /.../bsmr_lab/buck-out/v2/gen/root/200212f73efcd57d/bsmr_lab/greeter_bin/__main__/main
```

</OssOnly>

export const TARGET_NAME = isInternal() ?
<code>upstream//scripts/$USER/bsmr_lab/greeter_bin:main</code> :
<code>root//bsmr_lab/greeter_bin:main</code>;

- `BUILD SUCCEEDED` indicates that Bessemer successfully built our target.
- {TARGET_NAME} is the full target label name of our target.
- Think of the `:main` we used in the step 2 as a relative path to the target
  from within its package (`greeter_bin`).
- The full name {TARGET_NAME} is like an absolute path, uniquely identifying the
  target within your entire project (fbsource).
- `buck-out/.../__main__/main` is the path of our binary output. It is the
  relative path to fbsource. You can use `--show-full-output` instead of
  `--show-output` to get the absolute path.

## Step 5: Running Your Application with Bessemer

Since our target is a runnable target, we can run it by `bsmr run`

<FbInternalOnly>

1. In your terminal (still in the fbsource root directory), execute:

```bash
bsmr run upstream//scripts/$USER/bsmr_lab/greeter_bin:main
```

</FbInternalOnly>

<OssOnly>

1. In your terminal (still in the `greeter_bin` directory), execute:

```bash
bsmr run :main
```

</OssOnly>

This command tells Bessemer to run the `main` target. Bessemer will build it if it
hasn't been built already, and then execute it. (i.e. We can do this without
step 4)

2. Expected Output: You will see:

```
Hello world!
```

There it is! Our program ran successfully and printed the message.

## Step 6: Inspecting Your Target (Optional)

This step is optional, but it's good to know how you can ask Bessemer about the
targets you've defined.

<FbInternalOnly>

1. To see the target that we defined (still in the fbsource root directory),
   run:

```bash
bsmr targets upstream//scripts/$USER/bsmr_lab/greeter_bin:
```

</FbInternalOnly>

<OssOnly>

1. To see the target that we defined (still in the `greeter_bin` directory),
   run:

```bash
bsmr targets :
```

</OssOnly>

2. Expected Output:

This will show all the targets we defined,

<FbInternalOnly>

```
upstream//scripts/$USER/bsmr_lab/greeter_bin:main
... other targets might be listed here ...
```

You would see other targets as well, but for now we can just ignore them and
just focus on the `main` target.

</FbInternalOnly>

<OssOnly>

```
root//bsmr_lab/greeter_bin:main
```

</OssOnly>

## Conclusion

Congratulations! 🎉

You have successfully created, built, and run your first application using
Bessemer! We've walked through

- Setting up the project structure.
- Writing a simple Rust program.
- Defining a rust_binary target in a BUILD.bsmr file.
- Using `bsmr build` to compile the code.
- Using `bsmr run` to execute the program.
- Using `bsmr targets` to inspect the target.

You've taken your first concrete steps into the world of Bessemer.
