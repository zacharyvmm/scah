
enum PairingState {
    NewKey,
    AssignValue,
}

type Pairing<'a> = Vec<(&'a str, Option<&'a str>)>;

pub struct Pair<'a> {
    state: PairingState,
    key_buf: Option<&'a str>,

    // Key Value pair. The Value can be null
    pairs: Pairing<'a>,
}

impl<'a> Pair<'a> {
    pub fn new() -> Self {
        return Pair {
            state: PairingState::NewKey,
            key_buf: None,
            pairs: Vec::new(),
        };
    }
    
    pub fn get_pairs(&self) -> &Pairing<'a> {
        return &self.pairs;
    }

    pub fn set_to_new_key(&mut self) -> () {
        if let Some(key) = self.key_buf {
            self.pairs.push((key, None));
            self.key_buf = None;
        }

        self.state = PairingState::NewKey;
    }

    pub fn add_string(&mut self, content: &'a str) {
        match self.state {
            PairingState::NewKey => {
                // This should not happen, but better safe then sorry
                // self.set_to_NewKey();

                self.key_buf = Some(content);

                self.state = PairingState::AssignValue;
            }

            PairingState::AssignValue => {
                if let Some(key) = self.key_buf {
                    self.pairs.push((key, Some(content)));
                    self.key_buf = None;
                }

                self.state = PairingState::NewKey;
            }
        }
    }
}