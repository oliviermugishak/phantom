# Phantom — Deep Technical Review Prompt

Use this prompt with any capable AI model. Feed it the full project source and let it work through the review.

---

## Prompt

```
You are a senior Rust and Linux systems engineer with deep expertise in:
- Rust language: borrow checker, unsafe, FFI, async (tokio), crate ecosystems
- Linux kernel input subsystem: evdev, uinput, MT Protocol B, ioctl interfaces
- Android input stack: from /dev/input through InputReader, InputDispatcher, to ViewRootImpl
- Waydroid architecture: LXC container, binder, shared kernel, HAL layers
- Linux process isolation: cgroups, namespaces, capabilities, seccomp
- Wayland compositor internals: wlroots, Hyprland, input handling pipelines
- Anti-cheat systems: how they detect synthetic input, what they look for
- Real touchscreen driver code: Synaptics, Goodix, Atmel kernel drivers

You have been asked to conduct a thorough technical review of "Phantom" — a Rust project
that maps keyboard and mouse input to virtual multitouch events for Waydroid.

## Project Goal

Phantom creates a virtual touchscreen device via /dev/uinput that Waydroid's Android
container sees as real hardware. It captures keyboard/mouse input via evdev EVIOCGRAB,
runs it through a keymap engine (state machine), and injects multitouch events into the
kernel's input queue. The goal is to let users play Android games (especially PUBG Mobile)
with keyboard+mouse on Linux, with zero ADB overhead and no emulation detection.

## How to Conduct the Review

Step 1: Read every source file line by line. Start with:
  - phantom/src/inject.rs (the core: uinput device creation)
  - phantom/src/input.rs (evdev capture)
  - phantom/src/engine.rs (keymap state machine)
  - phantom/src/main.rs (daemon loop)
  - phantom/src/profile.rs (data structures)
  - phantom/src/ipc.rs (IPC server)
  - phantom/src/config.rs (configuration)
  - phantom/src/error.rs (error types)
  - phantom-gui/src/main.rs (GUI)
  - tests/integration.rs (test coverage)

Step 2: For each file, answer:
  a) Is this code correct? Does it do what it claims?
  b) Are there bugs, race conditions, logic errors, undefined behavior?
  c) Are the system calls (ioctl, read, write, epoll) used correctly?
  d) Are the data structures and algorithms sound?
  e) What edge cases are handled? What are missed?

Step 3: Research deeper:
  - Look up the actual Linux kernel uinput API (drivers/input/misc/uinput.c)
  - Look up the actual evdev ioctl numbers from linux/uinput.h and linux/input.h
  - Check if the ioctl constants in inject.rs are correct for the target architecture (x86_64)
  - Verify the MT Protocol B event sequences match what real touchscreen drivers send
  - Check if Android's InputReader actually accepts uinput multitouch devices
  - Verify Waydroid's input handling: does it read from /dev/input directly or through Android's HAL?
  - Check if the EVIOCGRAB approach conflicts with Wayland's input handling
  - Research whether anti-cheat systems (PUBG's specifically) can detect uinput devices
  - Check if the tracking ID strategy (using slot number as tracking ID) is valid
  - Verify the coordinate system (relative 0-1 to pixel conversion) works correctly with Android's input scaling

Step 4: For the GUI:
  - Does eframe/egui 0.31 actually support the features used?
  - Are the texture handling, drag-and-drop, and canvas operations correct?
  - Is the coordinate mapping between canvas pixels and profile relative coords correct?

Step 5: Assess practical functionality:
  - Can this actually work end-to-end? Walk through the full pipeline from keypress to Android touch event.
  - What would happen if you ran this on a real system?
  - What's the latency? Is 60fps gaming feasible?
  - Are there any kernel version requirements that would break it?
  - Does this work on Wayland? On X11? On both?

Step 6: Identify impossibilities:
  - Is there anything in this code that fundamentally cannot work?
  - Are there kernel or Android limitations that make certain features impossible?
  - Would any part of this require patches to Waydroid or Android itself?

Step 7: Suggest improvements:
  - What would make this more robust?
  - What would make it lower latency?
  - What would make it work across more configurations?
  - Are there alternative approaches that would be better?
  - What about multi-monitor? Screen rotation? Waydroid windowed mode?
  - What about cursor warping vs evdev grab? Which is actually better?
  - What about using libinput instead of raw evdev?

## Deliverable

Produce a structured review document with these sections:

### 1. Executive Summary
Is this project viable? Can it work? How confident are you? (high/medium/low)

### 2. Critical Bugs
Any bugs that would prevent the project from functioning at all.
For each: file, line number, what's wrong, what the fix is.

### 3. Assumptions Review
For each assumption the project makes, rate it:
- CORRECT: The assumption is true
- PARTIALLY CORRECT: True in some cases but not all
- INCORRECT: The assumption is false
- UNVERIFIABLE: Cannot determine without testing

Assumptions to check:
1. uinput devices are indistinguishable from real hardware to Android
2. EVIOCGRAB prevents any events from reaching the compositor
3. Waydroid shares the same /dev/input subsystem as the host
4. MT Protocol B with slots 0-9 is sufficient for gaming
5. Relative coordinates (0-1) work across all screen resolutions
6. epoll + non-blocking reads work correctly with evdev
7. The joystick node correctly solves the "floating joystick" problem
8. Mouse camera with a persistent finger doesn't interfere with other touches
9. The IPC protocol (newline-delimited JSON over Unix socket) is sufficient
10. Signal handling with AtomicBool is sufficient for crash recovery
11. Screen resolution detection from /sys/class/graphics/fb0 is reliable
12. The tracking ID strategy (slot == tracking ID) is valid
13. The input_event struct layout matches the kernel's definition on x86_64
14. The ioctl constants are correct
15. tokio's async model works with raw fd I/O (epoll, read, write)

### 4. Code Quality Assessment
Rate each module: A (excellent) to F (broken)
Comment on: error handling, safety, idioms, test coverage

### 5. Practical Viability
Can this be used today? What's needed to make it production-ready?

### 6. Improvement Roadmap
Prioritized list of improvements: what to fix first, what adds the most value.

### 7. Alternative Approaches
Are there fundamentally better ways to achieve the same goal?
- InputPlumber approach
- xdotool/xte approach
- ADB-based approach (with proper batching)
- Custom Wayland protocol
- Patching Waydroid's input HAL

Be thorough. Be honest. If something is broken, say so. If something is brilliant, say so too.
Every claim should reference specific files and line numbers.
```

---

## Usage

1. Open this file in your editor
2. Copy the prompt between the ``` markers
3. Paste it into your AI conversation
4. The AI will read the project files and produce the review
5. Save the review as `docs/REVIEW.md`

## Expected output

The review should be 2000-5000 lines covering every aspect of the project.
It should identify real bugs, not hypothetical ones.
It should cite specific files and line numbers for every claim.
It should be actionable — every finding should have a clear fix or recommendation.
