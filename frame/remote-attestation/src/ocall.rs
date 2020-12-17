use anyhow::Result;
use frame_types::UntrustedStatus;
use sgx_types::*;

extern "C" {
    pub fn ocall_sgx_init_quote(
        retval: *mut UntrustedStatus,
        ret_ti: *mut sgx_target_info_t,
        ret_gid: *mut sgx_epid_group_id_t,
    ) -> sgx_status_t;

    pub fn ocall_get_quote(
        retval: *mut UntrustedStatus,
        p_sigrl: *const u8,
        sigrl_len: u32,
        report: *const sgx_report_t,
        quote_type: sgx_quote_sign_type_t,
        p_spid: *const sgx_spid_t,
        p_nonce: *const sgx_quote_nonce_t,
        p_qe_report: *mut sgx_report_t,
        p_quote: *mut sgx_quote_t,
        maxlen: u32,
        p_quote_len: *mut u32,
    ) -> sgx_status_t;
}

pub fn sgx_init_quote() -> Result<sgx_target_info_t> {
    let mut rt = UntrustedStatus::default();
    let mut target_info = sgx_target_info_t::default();
    let mut gid = sgx_epid_group_id_t::default();

    let status = unsafe {
        ocall_sgx_init_quote(
            &mut rt as *mut UntrustedStatus,
            &mut target_info as *mut sgx_target_info_t,
            &mut gid as *mut sgx_epid_group_id_t,
        )
    };

    if status != sgx_status_t::SGX_SUCCESS {
        return Err(FrameEnclaveError::SgxError { err: status });
    }
    if rt.is_err() {
        return Err(FrameEnclaveError::UntrustedError {
            status: rt,
            function: "ocall_sgx_init_quote",
        });
    }

    Ok(target_info)
}

pub fn get_quote(report: sgx_report_t, spid: &sgx_spid_t) -> Result<Vec<u8>> {
    const RET_QUOTE_BUF_LEN: u32 = 2048;
    let mut quote_len: u32 = 0;
    let mut rt = UntrustedStatus::default();
    let mut quote = vec![0u8; RET_QUOTE_BUF_LEN as usize];

    let status = unsafe {
        ocall_get_quote(
            &mut rt as *mut UntrustedStatus,
            std::ptr::null(), // p_sigrl
            0,                // sigrl_len
            &report as *const sgx_report_t,
            sgx_quote_sign_type_t::SGX_UNLINKABLE_SIGNATURE, // quote_type
            spid as *const sgx_spid_t,                       // p_spid
            std::ptr::null(),                                // p_nonce
            std::ptr::null_mut(),                            // p_qe_report
            quote.as_mut_ptr() as *mut sgx_quote_t,
            RET_QUOTE_BUF_LEN, // maxlen
            &mut quote_len as *mut u32,
        )
    };

    if status != sgx_status_t::SGX_SUCCESS {
        return Err(FrameEnclaveError::SgxError { err: status });
    }
    if rt.is_err() {
        return Err(FrameEnclaveError::UntrustedError {
            status: rt,
            function: "ocall_get_quote",
        });
    }

    let _ = quote.split_off(quote_len as usize);
    Ok(quote)
}
