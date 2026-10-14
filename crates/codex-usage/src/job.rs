use crate::UsageError;
use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::null,
};
use windows_sys::Win32::{
    Foundation::*,
    System::{JobObjects::*, Threading::*},
};

/// Windows closes this handle even if the widget crashes, terminating its query process.
pub(crate) struct ChildJob(usize);

impl ChildJob {
    pub(crate) fn attach(pid: u32) -> Result<Self, UsageError> {
        let failure = || UsageError::new("query_failed", "无法创建额度查询进程，请检查系统限制。");
        unsafe {
            let handle = CreateJobObjectW(null(), null());
            if handle.is_null() {
                return Err(failure());
            }
            let job = Self(handle as usize);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const c_void,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == 0
            {
                return Err(failure());
            }
            let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if process.is_null() {
                return Err(failure());
            }
            let success = AssignProcessToJobObject(handle, process) != 0;
            CloseHandle(process);
            if !success {
                return Err(failure());
            }
            Ok(job)
        }
    }
}

impl Drop for ChildJob {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0 as HANDLE);
        }
    }
}
