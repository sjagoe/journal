#[derive(Debug, PartialEq, Eq, Hash)]
enum KeyMapAction {
    Action(Action),
    Input(InputChain),
}

impl KeyMapAction {
    fn to_owned(&self) -> KeyMapAction {
        match self {
            KeyMapAction::Action(action) => KeyMapAction::Action(action.clone()),
            KeyMapAction::Input(input) => KeyMapAction::Input(input.to_owned()),
        }
    }
}

impl Display for KeyMapAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KeyMapAction::Action(action) => write!(f, "{:?}", action),
            KeyMapAction::Input(input) => write!(f, "{}", input),
        }
    }
}

type KeyMapType = HashMap<InputChain, KeyMapAction>;
type KeyMapReverseType = HashMap<KeyMapAction, InputChain>;

#[derive(Debug)]
struct KeyMap {
    map: KeyMapType,
    reverse: KeyMapReverseType,
}

impl KeyMap {
    pub fn keybindings(&self) -> Vec<String> {
        let mut bindings: Vec<String> = Vec::new();

        for action in [
            Action::AppExit,
            Action::ExportEntry,
            Action::NameNewEntry,
            Action::ActivateSelectedEntry,
            Action::Refresh,
            Action::NavigateNext,
            Action::NavigatePrevious,
            Action::NavigatePageUp,
            Action::NavigatePageDown,
            Action::NavigateFirst,
            Action::NavigateLast,
            Action::NavigateSwitchPane,
            Action::NavigateBack,
            Action::GitStageFile,
            Action::GitUnstageFile,
            Action::GitCommitMessage,
        ] {
            let key_action = KeyMapAction::Action(action.clone());
            if let Some(binding) = self.reverse.get(&key_action) {
                let binding_str = binding.to_string();
                bindings.push(format!("{}: {}", binding_str, action));
            }
        }

        bindings
    }

    fn add_keymap(
        keys: &mut KeyMapType,
        reverse: &mut KeyMapReverseType,
        input: &InputChain,
        action: &Action,
    ) {
        for (input, next_input) in input.get_chain().iter() {
            if let Some(next_input) = next_input {
                let next = KeyMapAction::Input(next_input.to_owned());
                keys.insert(input.to_owned(), next);
            } else {
                let map_action = KeyMapAction::Action(action.clone());
                keys.insert(input.to_owned(), map_action.to_owned());
                reverse
                    .entry(map_action)
                    .or_insert_with(|| input.to_owned());
            }
        }
    }

    #[allow(unused)]
    fn new(mapconfig: &HashMap<String, String>) -> Result<Self, JournalError> {
        let mut keys: KeyMapType = HashMap::new();
        let mut reverse: KeyMapReverseType = HashMap::new();

        for (keyseq, action_name) in mapconfig.iter() {
            let input_chain = InputChain::from_str(keyseq)?;
            let action = Action::from_str(action_name)?;
            Self::add_keymap(&mut keys, &mut reverse, &input_chain, &action);
        }

        let map = KeyMap { map: keys, reverse };
        Ok(map)
    }

    pub(self) fn action(&self, input: &InputChain) -> KeyMapAction {
        if let Some(action) = self.map.get(input) {
            action.to_owned()
        } else {
            KeyMapAction::Action(Action::NoOp)
        }
    }
}

impl Display for KeyMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut lines: Vec<String> = Vec::new();
        for (input, action) in self.map.iter() {
            let line = format!("{}: {}", input, action);
            lines.push(line);
        }
        write!(f, "{}", lines.join("\n"))
    }
}

const DEFAULT_KEYS: [(&str, Action); 20] = [
    ("C-x", Action::AppExit),
    ("C-e", Action::ExportEntry),
    ("C-g", Action::NavigateBack),
    ("C-n", Action::NavigateNext),
    ("C-p", Action::NavigatePrevious),
    ("C-v", Action::NavigatePageDown),
    ("M-v", Action::NavigatePageUp),
    ("M-n", Action::NameNewEntry),
    ("M-s", Action::GitStageFile),
    ("M-u", Action::GitUnstageFile),
    ("M-c", Action::GitCommitMessage),
    ("g", Action::Refresh),
    ("Enter", Action::ActivateSelectedEntry),
    ("Home", Action::NavigateFirst),
    ("End", Action::NavigateLast),
    ("PageUp", Action::NavigatePageUp),
    ("PageDown", Action::NavigatePageDown),
    ("Up", Action::NavigatePrevious),
    ("Down", Action::NavigateNext),
    ("Tab", Action::NavigateSwitchPane),
];

impl Default for KeyMap {
    fn default() -> Self {
        let mut keys: KeyMapType = HashMap::new();
        let mut reverse: KeyMapReverseType = HashMap::new();

        for (keyseq, action) in DEFAULT_KEYS {
            let input_chain = InputChain::from_str(keyseq).unwrap();
            Self::add_keymap(&mut keys, &mut reverse, &input_chain, &action);
        }

        KeyMap { map: keys, reverse }
    }
}
