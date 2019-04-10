extern crate core;
extern crate libc;

use flat_absy::flat_variable::FlatVariable;
use proof_system::ProofSystem;
use std::fs::File;
use std::io::{BufReader, Write};
use zkstandard::{
    flatbuffers::{FlatBufferBuilder, WIPOffset},
    zkinterface_generated::zkinterface::{
        AssignedVariables,
        AssignedVariablesArgs,
        BilinearConstraint,
        BilinearConstraintArgs, GadgetReturn,
        GadgetReturnArgs,
        Message,
        Root,
        RootArgs,
        VariableValues,
        VariableValuesArgs,
    },
};
use zokrates_field::field::{Field, FieldPrime};

pub struct ZkInterface {}

impl ZkInterface {
    pub fn new() -> ZkInterface {
        ZkInterface {}
    }
}

impl ProofSystem for ZkInterface {
    fn setup(
        &self,
        variables: Vec<FlatVariable>,
        a: Vec<Vec<(usize, FieldPrime)>>,
        b: Vec<Vec<(usize, FieldPrime)>>,
        c: Vec<Vec<(usize, FieldPrime)>>,
        num_inputs: usize,
        pk_path: &str,
        _vk_path: &str,
    ) -> bool {
        let n_outputs = 1;
        let _n_inputs = num_inputs - n_outputs;
        let free_variable_id_after = variables.len() as u64;
        let n_constraints = a.len();

        // Write R1CSConstraints message.
        write_r1cs(n_constraints, &a, &b, &c, pk_path);

        // Write Return message including free_variable_id_after.
        write_return(free_variable_id_after, None, &format!("return_{}", pk_path));

        true
    }

    fn generate_proof(
        &self,
        _pk_path: &str,
        proof_path: &str,
        public_inputs: Vec<FieldPrime>,
        local_values: Vec<FieldPrime>,
    ) -> bool {
        let n_outputs = 1;
        let n_inputs = public_inputs.len() - n_outputs;
        let free_variable_id_before = public_inputs.len() as u64;
        let free_variable_id_after = (public_inputs.len() + local_values.len()) as u64;

        println!("{:?}", public_inputs);

        // Write assignment to local variables.
        write_assignment(free_variable_id_before, &local_values, proof_path);

        // Write Return message including output values.
        let outputs = &public_inputs[n_inputs..];
        write_return(free_variable_id_after, Some(outputs), &format!("return_{}", proof_path));

        true
    }

    fn export_solidity_verifier(&self, _reader: BufReader<File>) -> String {
        format!(
            "func export_solidity_verifier is not implemented",
        );

        return String::from("func export_solidity_verifier is not implemented");
    }
}


fn write_r1cs(
    num_constraints: usize,
    a: &Vec<Vec<(usize, FieldPrime)>>,
    b: &Vec<Vec<(usize, FieldPrime)>>,
    c: &Vec<Vec<(usize, FieldPrime)>>,
    to_path: &str,
) {
    let mut builder = zkstandard::flatbuffers::FlatBufferBuilder::new();

    // create vector of
    let mut vector_lc = vec![];

    for i in 0..num_constraints {
        let a_var_val = convert_linear_combination(&mut builder, &a[i]);
        let b_var_val = convert_linear_combination(&mut builder, &b[i]);
        let c_var_val = convert_linear_combination(&mut builder, &c[i]);

        let lc = BilinearConstraint::create(&mut builder, &BilinearConstraintArgs {
            linear_combination_a: Some(a_var_val),
            linear_combination_b: Some(b_var_val),
            linear_combination_c: Some(c_var_val),
        });
        vector_lc.push(lc);
    }

    let vector_offset = builder.create_vector(vector_lc.as_slice());

    let args = zkstandard::zkinterface_generated::zkinterface::R1CSConstraintsArgs { constraints: Some(vector_offset) };

    let r1cs_constraints = zkstandard::zkinterface_generated::zkinterface::R1CSConstraints::create(&mut builder, &args);
    let root_args = zkstandard::zkinterface_generated::zkinterface::RootArgs { message_type: zkstandard::zkinterface_generated::zkinterface::Message::R1CSConstraints, message: Some(r1cs_constraints.as_union_value()) };
    let root = zkstandard::zkinterface_generated::zkinterface::Root::create(&mut builder, &root_args);

    builder.finish_size_prefixed(root, None);

    println!("Writing {}", to_path);
    let mut file = File::create(to_path).unwrap();
    file.write_all(builder.finished_data()).unwrap();
}

fn convert_linear_combination<'a>(builder: &mut FlatBufferBuilder<'a>, item: &Vec<(usize, FieldPrime)>) -> (WIPOffset<VariableValues<'a>>) {
    let mut var_ids: Vec<u64> = Vec::new();
    let mut elements: Vec<u8> = Vec::new();

    for i in 0..item.len() {
        var_ids.push(item[i].0 as u64);

        let mut bytes = item[i].1.into_byte_vector().clone();
        elements.append(&mut bytes);
    }

    let var_ids_vector = builder.create_vector(&var_ids);
    let elements_vector = builder.create_vector(&elements);

    VariableValues::create(builder, &VariableValuesArgs {
        variable_ids: Some(var_ids_vector),
        elements: Some(elements_vector),
    })
}


fn write_assignment(
    free_variable_id_before: u64,
    local_values: &[FieldPrime],
    to_path: &str,
) {
    let mut builder = &mut FlatBufferBuilder::new();

    let mut ids = vec![];
    let mut elements = vec![];
    for i in 0..local_values.len() {
        ids.push(free_variable_id_before + i as u64);

        let mut bytes = local_values[i].into_byte_vector();
        elements.append(&mut bytes);
    }

    let elements = builder.create_vector(&elements);
    let ids = builder.create_vector(&ids);
    let values = VariableValues::create(&mut builder, &VariableValuesArgs {
        variable_ids: Some(ids),
        elements: Some(elements),
    });
    let assign = AssignedVariables::create(&mut builder, &AssignedVariablesArgs {
        values: Some(values),
    });
    let message = Root::create(&mut builder, &RootArgs {
        message_type: Message::AssignedVariables,
        message: Some(assign.as_union_value()),
    });
    builder.finish_size_prefixed(message, None);

    println!("Writing {}", to_path);
    let mut file = File::create(to_path).unwrap();
    file.write_all(builder.finished_data()).unwrap();
}


fn write_return(
    free_variable_id_after: u64,
    outputs: Option<&[FieldPrime]>,
    to_path: &str,
) {
    let mut builder = &mut FlatBufferBuilder::new();

    let outgoing_elements = if let Some(outputs) = outputs {
        // Convert output element representations.
        let mut elements = vec![];
        for o in outputs {
            let mut bytes = o.into_byte_vector();
            elements.append(&mut bytes);
        }
        Some(builder.create_vector(&elements))
    } else {
        None
    };

    let gadret = GadgetReturn::create(&mut builder, &GadgetReturnArgs {
        free_variable_id_after,
        outgoing_elements,
        info: None,
        error: None,
    });
    let message = Root::create(&mut builder, &RootArgs {
        message_type: Message::GadgetReturn,
        message: Some(gadret.as_union_value()),
    });
    builder.finish_size_prefixed(message, None);

    println!("Writing {}", to_path);
    let mut file = File::create(to_path).unwrap();
    file.write_all(builder.finished_data()).unwrap();
}
