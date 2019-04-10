extern crate zkstandard;

use self::zkstandard::assignment_request::make_assignment_request;
use self::zkstandard::gadget_call::InstanceDescription;
use self::zkstandard::r1cs_request::make_r1cs_request;
use self::zkstandard::r1cs_request::R1CSContext;
use zokrates_field::field::Field;


fn make_sha256_instance() -> InstanceDescription {
    InstanceDescription {
        gadget_name: "sha256".to_string(),
        incoming_variable_ids: vec![1, 2],
        outgoing_variable_ids: Some(vec![3]),
        free_variable_id_before: 4,
        field_order: None,
    }
}

pub fn get_sha256_witness<T: Field>(inputs: &Vec<T>) -> Vec<T> {
    let instance = make_sha256_instance();

    let in_elements: Vec<Vec<u8>> = inputs.iter().map(|f| f.into_byte_vector()).collect();
    let in_elements = in_elements.iter().map(|e| e as &[u8]).collect();

    let assign_ctx = make_assignment_request(instance, in_elements);

    assign_ctx.iter_assignment().map(
        |a| T::from_byte_vector(Vec::from(a.element))
    ).collect()
}

pub fn get_sha256_constraints() -> R1CSContext {
    let instance = make_sha256_instance();
    make_r1cs_request(instance)
}
