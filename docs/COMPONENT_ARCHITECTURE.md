# Component Architecture

This document explains Thoth's component architecture and the one-way data binding pattern used for building UI components.

## Overview

Thoth uses a **trait-based component system** inspired by React's component model, adapted to work with Rust's ownership system and egui's immediate mode GUI pattern.

## Component Traits

We have three main component traits, each serving a different purpose:

### 1. StatelessComponent

Pure functional components that don't maintain state between renders.

```rust
pub trait StatelessComponent {
    type Props;
    type Output;

    fn render(ui: &mut egui::Ui, props: Self::Props) -> Self::Output;
}
```

**Use cases**: Simple buttons, labels, icons, static UI elements

**Example**:
```rust
struct IconButton;

impl StatelessComponent for IconButton {
    type Props = (&'static str, &'static str); // (icon, tooltip)
    type Output = bool; // clicked

    fn render(ui: &mut egui::Ui, props: Self::Props) -> Self::Output {
        let (icon, tooltip) = props;
        ui.button(icon).on_hover_text(tooltip).clicked()
    }
}
```

### 2. StatefulComponent

Components that maintain internal state between renders.

```rust
pub trait StatefulComponent {
    type Output;

    fn render(&mut self, ui: &mut egui::Ui) -> Self::Output;
}
```

**Use cases**: Text inputs, counters, expandable sections, file viewers

**Example**:
```rust
struct Counter {
    value: i32,
}

impl StatefulComponent for Counter {
    type Output = ();

    fn render(&mut self, ui: &mut egui::Ui) -> Self::Output {
        ui.horizontal(|ui| {
            if ui.button("-").clicked() {
                self.value -= 1;
            }
            ui.label(format!("{}", self.value));
            if ui.button("+").clicked() {
                self.value += 1;
            }
        });
    }
}
```

### 3. ContextComponent

Components that need access to the full `egui::Context` to create top-level panels.

```rust
pub trait ContextComponent {
    type Props<'a>;
    type Output;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output;
}
```

**Use cases**: Toolbar, Settings panel, Central panel, any top-level UI

## One-Way Data Binding Pattern

Our component architecture follows a **one-way data binding** pattern similar to React:

```
┌─────────────┐
│   Parent    │
│ Component   │
└─────────────┘
      │
      │ Props (immutable)
      ▼
┌─────────────┐
│    Child    │
│ Component   │
└─────────────┘
      │
      │ Events (actions)
      ▼
┌─────────────┐
│   Parent    │
│   Handles   │
└─────────────┘
```

### Data Flow

1. **Props flow down** (parent → child): Immutable data passed as references
2. **Events flow up** (child → parent): Actions/events returned in Output

This pattern avoids Rust's borrow checker conflicts that arise when trying to use callbacks with mutable closures.

## Example: Toolbar Component

Let's walk through a complete example using the Toolbar component.

### Step 1: Define Props

Props are immutable data passed from parent to child:

```rust
pub struct ToolbarProps<'a> {
    pub file_path: &'a Option<PathBuf>,
    pub file_type: &'a FileType,
    pub dark_mode: bool,
    pub show_settings: bool,
    pub update_available: bool,
    pub shortcuts: &'a KeyboardShortcuts,
}
```

**Key points**:
- Use references (`&'a`) for borrowed data
- Use owned types (`bool`, small types) for values
- All fields are immutable from child's perspective

### Step 2: Define Events

Events represent actions that occurred in the child:

```rust
pub enum ToolbarEvent {
    FileOpen { path: PathBuf, file_type: FileType },
    FileClear,
    NewWindow,
    FileTypeChange(FileType),
    ToggleSettings,
    ToggleTheme,
}
```

**Key points**:
- Each variant represents a user action
- Include necessary data with the event
- Parent decides how to handle each event

### Step 3: Define Output

Output contains both immediate results and events:

```rust
pub struct ToolbarOutput {
    pub search_message: Option<SearchMessage>,
    pub events: Vec<ToolbarEvent>,
}
```

### Step 4: Implement the Component

```rust
impl ContextComponent for Toolbar {
    type Props<'a> = ToolbarProps<'a>;
    type Output = ToolbarOutput;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        let search_message = self.render_ui(ctx, props, &mut events);

        ToolbarOutput {
            search_message,
            events,
        }
    }
}

impl Toolbar {
    fn render_ui(
        &mut self,
        ctx: &egui::Context,
        props: ToolbarProps<'_>,
        events: &mut Vec<ToolbarEvent>,
    ) -> Option<SearchMessage> {
        // Read from props (immutable)
        let file_type = *props.file_type;
        let dark_mode = props.dark_mode;

        // Emit events when actions occur
        if ui.button("Clear").clicked() {
            events.push(ToolbarEvent::FileClear);
        }

        if ui.checkbox(&mut dark_mode_copy, "Dark").changed() {
            events.push(ToolbarEvent::ToggleTheme);
        }

        // Return immediate results
        Some(search_message)
    }
}
```

### Step 5: Use in Parent

```rust
fn render_toolbar(&mut self, ctx: &egui::Context) -> Option<SearchMessage> {
    // Render with props (data flows down)
    let output = self.window_state.toolbar.render(
        ctx,
        ToolbarProps {
            file_path: &self.window_state.file_path,
            file_type: &self.window_state.file_type,
            dark_mode: self.settings.dark_mode,
            show_settings: self.settings_panel.show,
            update_available: self.update_available,
            shortcuts: &self.settings.shortcuts,
        },
    );

    // Handle events (actions flow up)
    for event in output.events {
        match event {
            ToolbarEvent::FileOpen { path, file_type } => {
                self.window_state.file_path = Some(path);
                self.window_state.file_type = file_type;
                self.window_state.error = None;
            }
            ToolbarEvent::FileClear => {
                self.window_state.file_path = None;
                self.window_state.error = None;
            }
            ToolbarEvent::ToggleTheme => {
                self.settings.dark_mode = !self.settings.dark_mode;
            }
            // ... handle other events
        }
    }

    output.search_message
}
```

## Benefits of This Approach

### 1. Borrow Checker Friendly

Unlike callback-based approaches, this pattern doesn't create conflicting borrows:

```rust
// ❌ This doesn't work in Rust:
ToolbarProps {
    file_path: &self.file_path,           // immutable borrow
    on_clear: &mut || {
        self.file_path = None;            // mutable borrow - CONFLICT!
    }
}

// ✅ This works:
let output = toolbar.render(ctx, ToolbarProps {
    file_path: &self.file_path,           // immutable borrow
});
for event in output.events {              // mutable borrow later
    match event {
        ToolbarEvent::Clear => self.file_path = None,
    }
}
```

### 2. Type Safety

The compiler ensures all events are handled:

```rust
match event {
    ToolbarEvent::FileOpen { .. } => { /* ... */ }
    ToolbarEvent::FileClear => { /* ... */ }
    // Compiler error if you forget an event variant!
}
```

### 3. Testability

Easy to test components in isolation:

```rust
#[test]
fn test_toolbar_file_clear() {
    let mut toolbar = Toolbar::default();
    let props = ToolbarProps { /* ... */ };
    
    let output = toolbar.render(ctx, props);
    
    assert!(output.events.contains(&ToolbarEvent::FileClear));
}
```

### 4. Performance

- No unnecessary cloning of data
- Props are references (zero-cost)
- Events only created when actions occur

### 5. Familiar Pattern

Developers familiar with React will recognize this pattern:

| React | Thoth |
|-------|-------|
| Props (read-only) | `Props<'a>` struct |
| State | Component's internal fields |
| Callbacks | `Event` enum variants |
| `onChange={handler}` | Match on event enum |

## Best Practices

### 1. Keep Props Small

Only pass what the component needs:

```rust
// ✅ Good: Only relevant data
pub struct ButtonProps<'a> {
    pub label: &'a str,
    pub disabled: bool,
}

// ❌ Bad: Passing entire app state
pub struct ButtonProps<'a> {
    pub app_state: &'a AppState,
}
```

### 2. Use Descriptive Event Names

```rust
// ✅ Good: Clear intent
pub enum ToolbarEvent {
    FileOpen { path: PathBuf },
    FileTypeChanged(FileType),
    SearchRequested(String),
}

// ❌ Bad: Vague
pub enum ToolbarEvent {
    Action1(PathBuf),
    Changed(FileType),
    Event(String),
}
```

### 3. Include Necessary Data in Events

```rust
// ✅ Good: Event is self-contained
pub enum EditorEvent {
    TextChanged { new_text: String, cursor_pos: usize },
}

// ❌ Bad: Parent needs to fetch data
pub enum EditorEvent {
    TextChanged, // Parent has to query: "what's the new text?"
}
```

### 4. Don't Over-Event

Not every interaction needs an event:

```rust
// ✅ Good: Only emit events that affect parent state
if ui.button("Save").clicked() {
    events.push(EditorEvent::SaveRequested);
}

// ❌ Bad: Events for internal state
if ui.button("Hover me").hovered() {
    events.push(EditorEvent::ButtonHovered); // Parent doesn't care!
}
```

### 5. Use Lifetimes Appropriately

```rust
// ✅ Good: Single lifetime for related references
pub struct Props<'a> {
    pub name: &'a str,
    pub items: &'a [Item],
}

// ❌ Usually unnecessary: Multiple lifetimes
pub struct Props<'a, 'b> {
    pub name: &'a str,
    pub items: &'b [Item],
}
```

## Comparison with Other Patterns

### vs. Callbacks (Why not `on_change: &mut dyn FnMut(...)`?)

**Callbacks have borrow checker issues**:

```rust
// ❌ Borrow checker conflict
let output = component.render(ctx, Props {
    value: &self.value,        // immutable borrow
    on_change: &mut |v| {
        self.value = v;        // mutable borrow - ERROR!
    }
});
```

**Events solve this**:

```rust
// ✅ No conflict - borrows happen at different times
let output = component.render(ctx, Props {
    value: &self.value,        // immutable borrow ends here
});
for event in output.events {   // mutable borrow starts here
    self.value = event.new_value;
}
```

### vs. Message Passing (Why not channels?)

Channels add unnecessary complexity for UI:

```rust
// ❌ Overkill for immediate UI events
let (tx, rx) = mpsc::channel();
component.render(ctx, Props { sender: tx });
while let Ok(event) = rx.try_recv() {
    handle_event(event);
}

// ✅ Simpler and more direct
let output = component.render(ctx, props);
for event in output.events {
    handle_event(event);
}
```

### vs. Direct Mutation (Why not pass `&mut` props?)

Breaking encapsulation:

```rust
// ❌ Child directly mutates parent state
component.render(ctx, Props {
    value: &mut self.value,  // Child can change it directly
});

// ✅ Parent controls when/how state changes
let output = component.render(ctx, Props {
    value: &self.value,      // Read-only
});
for event in output.events {
    // Parent decides what to do
    self.value = transform(event);
}
```

## Migration Guide

When refactoring an existing component to use traits:

### Before (ad-hoc)

```rust
pub struct Toolbar { /* ... */ }

impl Toolbar {
    pub fn ui(&mut self, ctx: &egui::Context, state: &mut AppState) -> Output {
        // Directly mutates state
        if ui.button("Clear").clicked() {
            state.file_path = None;
        }
    }
}
```

### After (trait-based)

```rust
pub struct Toolbar { /* ... */ }

pub struct ToolbarProps<'a> {
    pub file_path: &'a Option<PathBuf>,
}

pub enum ToolbarEvent {
    FileClear,
}

pub struct ToolbarOutput {
    pub events: Vec<ToolbarEvent>,
}

impl ContextComponent for Toolbar {
    type Props<'a> = ToolbarProps<'a>;
    type Output = ToolbarOutput;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        
        if ui.button("Clear").clicked() {
            events.push(ToolbarEvent::FileClear);
        }
        
        ToolbarOutput { events }
    }
}

// In parent:
let output = toolbar.render(ctx, ToolbarProps {
    file_path: &state.file_path,
});
for event in output.events {
    match event {
        ToolbarEvent::FileClear => state.file_path = None,
    }
}
```

## Further Reading

- [Rust Book: Traits](https://doc.rust-lang.org/book/ch10-02-traits.html)
- [Generic Associated Types (GATs)](https://blog.rust-lang.org/2022/10/28/gats-stabilization.html)
- [React: Thinking in React](https://react.dev/learn/thinking-in-react)
- [egui Documentation](https://docs.rs/egui/latest/egui/)

## Examples in Codebase

- **Toolbar**: `src/components/toolbar.rs` - Full example of ContextComponent
- **FileFormatViewer**: `src/components/file_viewer/viewer_trait.rs` - Specialized trait pattern
- **Component Traits**: `src/components/traits.rs` - Trait definitions

## Contributing

When adding new components:

1. Choose the appropriate trait (Stateless, Stateful, or Context)
2. Define clear Props and Event types
3. Document the component's purpose and usage
4. Add examples to this document if it introduces new patterns

---

**Questions or suggestions?** Open an issue or discussion on GitHub!
