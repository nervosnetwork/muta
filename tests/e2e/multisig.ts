import { Address } from '@mutajs/types';
import { createBindingClass, write, Write } from '@mutajs/service';

type u8 = number;
type u32 = number;

interface AddressWithWeight {
    address: Address,
    weight: u8, // u8
}

interface GenerateMultiSigAccountPayload {
    owner: Address,
    addr_with_weight: Array<AddressWithWeight>,
    threshold: u32,
    memo: string,
}

interface GenerateMultiSigAccountResponse {
    address: Address,
}

export interface MultiSigServiceModel {
  generate_account: Write<GenerateMultiSigAccountPayload, GenerateMultiSigAccountResponse>;
}

export const MultiSigService = createBindingClass<MultiSigServiceModel>('multi_signature', {
    generate_account: write(),
});
