use std::cell::RefCell;
use std::io;
use std::rc::Rc;

use bytes::Bytes;
use ckb_vm::{DefaultMachineBuilder, SupportMachine};

// use protocol::traits::ServiceSDK;
use protocol::traits::{ServiceSDK, StoreMap};
use protocol::types::{Address, Hash, ServiceContext};

use crate::types::{ExecPayload, InterpreterResult, InterpreterType};
use crate::vm;
use crate::vm::ChainInterface;

// Duktape execution environment
const DUKTAPE_EE: &[u8] = std::include_bytes!("c/duktape_ee");

#[derive(Clone, Debug)]
pub struct InterpreterConf {
    pub print_debug: bool,
}

impl Default for InterpreterConf {
    fn default() -> Self {
        InterpreterConf { print_debug: true }
    }
}

#[derive(Clone, Debug)]
pub struct InterpreterParams {
    pub address: Address,
    pub code:    Bytes,
    pub args:    Bytes,
    pub is_init: bool,
}

pub struct Interpreter {
    pub context: ServiceContext,
    pub cfg:     InterpreterConf,
    pub r#type:  InterpreterType,
    pub iparams: InterpreterParams,
    pub chain:   Rc<RefCell<dyn ChainInterface>>,
}

impl Interpreter {
    pub fn new(
        context: ServiceContext,
        cfg: InterpreterConf,
        r#type: InterpreterType,
        iparams: InterpreterParams,
        chain: Rc<RefCell<dyn ChainInterface>>,
    ) -> Self {
        Self {
            context,
            cfg,
            r#type,
            iparams,
            chain,
        }
    }

    pub fn run(&mut self) -> Result<InterpreterResult, ckb_vm::Error> {
        let output: Box<dyn io::Write> = if self.cfg.print_debug {
            Box::new(io::stdout())
        } else {
            Box::new(io::sink())
        };

        let (code, init_payload) = match self.r#type {
            InterpreterType::Binary => (self.iparams.code.clone(), None),
            InterpreterType::Duktape => (
                Bytes::from(DUKTAPE_EE.as_ref()),
                Some(self.iparams.code.clone()),
            ),
        };

        let mut args: Vec<Bytes> = vec!["main".into()];
        if let Some(payload) = init_payload {
            args.push(payload);
        }

        let ret_data = Rc::new(RefCell::new(Vec::new()));
        let core_machine =
            ckb_vm::DefaultCoreMachine::<u64, ckb_vm::SparseMemory<u64>>::new_with_max_cycles(
                self.context.get_cycles_limit(),
            );
        let mut machine = ckb_vm::DefaultMachineBuilder::<
            ckb_vm::DefaultCoreMachine<u64, ckb_vm::SparseMemory<u64>>,
        >::new(core_machine)
        .instruction_cycle_func(Box::new(vm::cost_model::instruction_cycles))
        .syscall(Box::new(vm::SyscallDebug::new("[ckb-vm debug]", output)))
        .syscall(Box::new(vm::SyscallEnvironment::new(
            self.context.clone(),
            self.iparams.clone(),
        )))
        .syscall(Box::new(vm::SyscallIO::new(
            self.iparams.args.to_vec(),
            ret_data.clone(),
        )))
        .syscall(Box::new(vm::SyscallChainInterface::new(self.chain.clone())))
        .build();
        machine.load_program(&code, &args[..]).unwrap();
        let exitcode = machine.run()?;
        let cycles = machine.cycles();
        let ret = ret_data.borrow();
        let result = InterpreterResult {
            ret_code:    exitcode,
            ret:         Bytes::from(ret.to_vec()),
            cycles_used: cycles,
        };
        Ok(result)
    }
}
