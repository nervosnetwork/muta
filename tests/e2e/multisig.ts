import { Address } from '@mutadev/types';
import { createServiceBindingClass, write, Write } from '@mutadev/service';

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

export const MultiSigService = createServiceBindingClass({
    serviceName: 'multi_signature',
    write: {
        generate_account: write<GenerateMultiSigAccountPayload, GenerateMultiSigAccountResponse>(),
    }
})
