use bytes::Bytes;
use derive_more::Constructor;
use rand::seq::SliceRandom;
use serde_derive::Deserialize;

use protocol::traits::Priority;

use crate::behaviors::{Behavior, MessageType, Request};
use crate::utils::{gen_bool, gen_range};

pub trait Strategy {
    fn get_behaviors(&self, request: Option<Request>) -> Vec<Behavior>;
}

#[derive(Constructor, Clone, Debug, Deserialize)]
pub struct BehaviorGenerator {
    pub req_end:     Option<String>,
    pub msg_type:    MessageType,
    pub probability: f64,
    pub num_range:   (u64, u64),
    pub priority:    Priority,
}

impl BehaviorGenerator {
    fn gen_behavior(
        &self,
        pub_key_list: &mut Vec<Bytes>,
        req: Option<Request>,
    ) -> Option<Behavior> {
        if gen_bool(self.probability) {
            let msg_num = gen_range(self.num_range.0, self.num_range.1);
            let send_to = gen_rand_pub_key_list(pub_key_list);
            let behavior =
                Behavior::new(self.msg_type.clone(), msg_num, req, send_to, self.priority);
            Some(behavior)
        } else {
            None
        }
    }
}

#[derive(Constructor, Clone, Debug)]
pub struct DefaultStrategy {
    pub_key_list: Vec<Bytes>,
    generators:   Vec<BehaviorGenerator>,
}

impl Strategy for DefaultStrategy {
    fn get_behaviors(&self, request: Option<Request>) -> Vec<Behavior> {
        let mut pub_key_list = self.pub_key_list.to_vec();
        self.generators
            .iter()
            .filter(|gen| {
                if request.is_none() {
                    gen.req_end.is_none()
                } else {
                    gen.req_end.is_some()
                        && gen.req_end.as_ref().unwrap() == request.as_ref().unwrap().to_end()
                }
            })
            .map(|gen| gen.gen_behavior(&mut pub_key_list, request.clone()))
            .filter_map(|opt| opt)
            .collect()
    }
}

pub fn gen_rand_pub_key_list(pub_key_list: &mut Vec<Bytes>) -> Vec<Bytes> {
    let mut rng = rand::thread_rng();
    pub_key_list.shuffle(&mut rng);

    let mut new_list = pub_key_list.to_vec();
    let cut_num = gen_range(0, new_list.len());
    new_list.split_off(cut_num)
}
