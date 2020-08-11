import { Address, Vec } from '@mutadev/types';
import { createServiceBindingClass, read, write } from '@mutadev/service';

type u8 = number;
type u32 = number;

interface AddressWithWeight {
    address: Address,
    weight: u8,
}

interface GenerateMultiSigAccountPayload {
    owner: Address,
    autonomy:         boolean,
    addr_with_weight: Vec<AddressWithWeight>,
    threshold: u32,
    memo: string,
}

interface GenerateMultiSigAccountResponse {
    address: Address,
}

interface GetMultiSigAccountPayload {
    multi_sig_address: Address
}

interface GetMultiSigAccountResponse {
    permissions: MultiSigPermission,
}

interface UpdateAccountPayload {
    account_address: Address,
    owner: Address,
    addr_with_weight: Vec<AddressWithWeight>,
    threshold: u32,
    memo: string,
}

interface MultiSigPermission {
    owner: Address,
    accounts: Vec<Account>,
    threshold: u32,
    memo: string,
}

interface Account {
    address: Address,
    weight: u8,
    is_multiple: Boolean,
}

export const MultiSigService = createServiceBindingClass({
    serviceName: 'multi_signature',
    read: {
        get_account_from_address: read<GetMultiSigAccountPayload, GetMultiSigAccountResponse>(),
    },
    write: {
        generate_account: write<GenerateMultiSigAccountPayload, GenerateMultiSigAccountResponse>(),
        update_account: write<UpdateAccountPayload, any>(),
    },
});
