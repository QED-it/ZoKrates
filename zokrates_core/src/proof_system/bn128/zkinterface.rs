extern crate core;
extern crate libc;

use flat_absy::flat_variable::FlatVariable;
use proof_system::ProofSystem;
use std::fs::File;
use std::io::{BufReader, Write};
use zkinterface::{
    flatbuffers::{FlatBufferBuilder, WIPOffset},
    writing::GadgetReturnSimple,
    zkinterface_generated::zkinterface::{
        AssignedVariables,
        AssignedVariablesArgs,
        BilinearConstraint,
        BilinearConstraintArgs,
        Message,
        R1CSConstraints,
        R1CSConstraintsArgs,
        Root,
        RootArgs,
        VariableValues,
        VariableValuesArgs,
    },
};
use zokrates_field::field::{Field, FieldPrime};
use zkinterface::writing::ConnectionSimple;

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
        num_public_inputs: usize,
        pk_path: &str,
        _vk_path: &str,
    ) -> bool {
        let num_inputs = 2;
        let first_output_id = 1 + num_inputs;
        let first_local_id = 1 + num_public_inputs as u64;
        let free_variable_id_after = variables.len() as u64;

        // Write R1CSConstraints message.
        write_r1cs(&a, &b, &c, pk_path);

        // Write Return message including free_variable_id_after.
        write_return(
            first_output_id,
            first_local_id,
            free_variable_id_after,
            None,
            &format!("return_{}", pk_path));

        true
    }

    fn generate_proof(
        &self,
        _pk_path: &str,
        proof_path: &str,
        public_inputs: Vec<FieldPrime>,
        local_values: Vec<FieldPrime>,
    ) -> bool {
        let num_inputs = 2;
        let first_output_id = 1 + num_inputs;
        let first_local_id = public_inputs.len() as u64;
        let free_variable_id_after = first_local_id + local_values.len() as u64;

        println!("{:?}", public_inputs);

        // Write assignment to local variables.
        write_assignment(first_local_id as u64, &local_values, proof_path);

        // Write Return message including output values.
        let outputs = &public_inputs[first_output_id as usize..];
        write_return(
            first_output_id,
            first_local_id,
            free_variable_id_after,
            Some(outputs),
            &format!("return_{}", proof_path));

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
    a: &Vec<Vec<(usize, FieldPrime)>>,
    b: &Vec<Vec<(usize, FieldPrime)>>,
    c: &Vec<Vec<(usize, FieldPrime)>>,
    to_path: &str,
) {
    let mut builder = FlatBufferBuilder::new();

    // create vector of
    let mut vector_lc = vec![];

    for i in 0..a.len() {
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

    let args = R1CSConstraintsArgs { constraints: Some(vector_offset) };

    let r1cs_constraints = R1CSConstraints::create(&mut builder, &args);
    let root_args = RootArgs { message_type: Message::R1CSConstraints, message: Some(r1cs_constraints.as_union_value()) };
    let root = Root::create(&mut builder, &root_args);

    builder.finish_size_prefixed(root, None);

    println!("Writing {}", to_path);
    let mut file = File::create(to_path).unwrap();
    file.write_all(builder.finished_data()).unwrap();
}

fn convert_linear_combination<'a>(builder: &mut FlatBufferBuilder<'a>, item: &Vec<(usize, FieldPrime)>) -> (WIPOffset<VariableValues<'a>>) {
    let mut variable_ids: Vec<u64> = Vec::new();
    let mut values: Vec<u8> = Vec::new();

    for i in 0..item.len() {
        variable_ids.push(item[i].0 as u64);

        let mut bytes = item[i].1.into_byte_vector();
        values.append(&mut bytes);
    }

    let variable_ids = Some(builder.create_vector(&variable_ids));
    let values = Some(builder.create_vector(&values));
    VariableValues::create(builder, &VariableValuesArgs {
        variable_ids,
        values,
    })
}


fn write_assignment(
    first_local_id: u64,
    local_values: &[FieldPrime],
    to_path: &str,
) {
    let mut builder = &mut FlatBufferBuilder::new();

    let mut ids = vec![];
    let mut values = vec![];

    for i in 0..local_values.len() {
        ids.push(first_local_id + i as u64);

        let mut bytes = local_values[i].into_byte_vector();
        values.append(&mut bytes);
    }

    let ids = builder.create_vector(&ids);
    let values = builder.create_vector(&values);
    let values = VariableValues::create(&mut builder, &VariableValuesArgs {
        variable_ids: Some(ids),
        values: Some(values),
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
    first_output_id: u64,
    first_local_id: u64,
    free_variable_id: u64,
    outputs: Option<&[FieldPrime]>,
    to_path: &str,
) {
    // Convert output element representations.
    let values = outputs.map(|outputs| {
        let mut values = vec![];
        for output in outputs {
            let mut bytes = output.into_byte_vector();
            values.append(&mut bytes);
        }
        values
    });

    let connection = ConnectionSimple {
        free_variable_id,
        variable_ids: (first_output_id..first_local_id).collect(),
        values,
    };

    let gadget_return = GadgetReturnSimple {
        outputs: connection,
    };

    let builder = &mut FlatBufferBuilder::new();
    let message = gadget_return.build(builder);
    builder.finish_size_prefixed(message, None);

    println!("Writing {}", to_path);
    let mut file = File::create(to_path).unwrap();
    file.write_all(builder.finished_data()).unwrap();
}
