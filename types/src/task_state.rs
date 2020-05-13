// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use crate::*;
use anyhow::{bail, ensure, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::convert::TryInto;
use uuid::Uuid;

const TASK_PREFIX: &str = "task";

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TaskState {
    pub task_id: Uuid,
    pub creator: UserID,
    pub function_id: ExternalID,
    pub function_arguments: FunctionArguments,
    pub executor: Executor,
    pub inputs_ownership: TaskFileOwners,
    pub outputs_ownership: TaskFileOwners,
    pub function_owner: UserID,
    pub participants: UserList,
    pub approved_users: UserList,
    pub assigned_inputs: TaskFiles<TeaclaveInputFile>,
    pub assigned_outputs: TaskFiles<TeaclaveOutputFile>,
    pub result: TaskResult,
    pub status: TaskStatus,
}

impl Storable for TaskState {
    fn key_prefix() -> &'static str {
        TASK_PREFIX
    }

    fn uuid(&self) -> Uuid {
        self.task_id
    }
}

impl TaskState {
    pub fn everyone_approved(&self) -> bool {
        // Single user task is by default approved by the creator
        (self.participants.len() == 1) || (self.participants == self.approved_users)
    }

    pub fn all_data_assigned(&self) -> bool {
        let input_args: HashSet<&String> = self.inputs_ownership.keys().collect();
        let assiged_inputs: HashSet<&String> = self.assigned_inputs.keys().collect();
        if input_args != assiged_inputs {
            return false;
        }

        let output_args: HashSet<&String> = self.outputs_ownership.keys().collect();
        let assiged_outputs: HashSet<&String> = self.assigned_outputs.keys().collect();
        if output_args != assiged_outputs {
            return false;
        }

        true
    }

    pub fn has_participant(&self, user_id: &UserID) -> bool {
        self.participants.contains(user_id)
    }

    pub fn has_creator(&self, user_id: &UserID) -> bool {
        &self.creator == user_id
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Task<S: StateTag> {
    state: TaskState,
    extra: S,
}

pub trait StateTag {}
impl StateTag for Create {}
impl StateTag for Assign {}
impl StateTag for Approve {}
impl StateTag for Stage {}
impl StateTag for Run {}
impl StateTag for Finish {}
impl StateTag for Done {}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Create;
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Assign;
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Approve;
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Stage;
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Run;
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Finish;
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Done;

impl std::convert::From<Create> for TaskStatus {
    fn from(_tag: Create) -> TaskStatus {
        TaskStatus::Created
    }
}

impl std::convert::From<Assign> for TaskStatus {
    fn from(_tag: Assign) -> TaskStatus {
        TaskStatus::Created
    }
}

impl std::convert::From<Approve> for TaskStatus {
    fn from(_tag: Approve) -> TaskStatus {
        TaskStatus::DataAssigned
    }
}

impl std::convert::From<Stage> for TaskStatus {
    fn from(_tag: Stage) -> TaskStatus {
        TaskStatus::Approved
    }
}

impl std::convert::From<Run> for TaskStatus {
    fn from(_tag: Run) -> TaskStatus {
        TaskStatus::Staged
    }
}

impl std::convert::From<Finish> for TaskStatus {
    fn from(_tag: Finish) -> TaskStatus {
        TaskStatus::Running
    }
}

impl std::convert::From<Done> for TaskStatus {
    fn from(_tag: Done) -> TaskStatus {
        TaskStatus::Finished
    }
}

impl Task<Create> {
    pub fn new(
        requester: UserID,
        req_executor: Executor,
        req_func_args: FunctionArguments,
        req_input_owners: impl Into<TaskFileOwners>,
        req_output_owners: impl Into<TaskFileOwners>,
        function: Function,
    ) -> Result<Self> {
        let req_input_owners = req_input_owners.into();
        let req_output_owners = req_output_owners.into();

        // gather all participants
        let input_owners = req_input_owners.all_owners();
        let output_owners = req_output_owners.all_owners();
        let mut participants = UserList::unions(vec![input_owners, output_owners]);
        participants.insert(requester.clone());
        if !function.public {
            participants.insert(function.owner.clone());
        }

        //check function compatibility
        let fn_args_spec: HashSet<&String> = function.arguments.iter().collect();
        let req_args: HashSet<&String> = req_func_args.inner().keys().collect();
        ensure!(fn_args_spec == req_args, "function_arguments mismatch");

        // check input fkeys
        let inputs_spec: HashSet<&String> = function.inputs.iter().map(|f| &f.name).collect();
        let req_input_fkeys: HashSet<&String> = req_input_owners.keys().collect();
        ensure!(inputs_spec == req_input_fkeys, "input keys mismatch");

        // check output fkeys
        let outputs_spec: HashSet<&String> = function.outputs.iter().map(|f| &f.name).collect();
        let req_output_fkeys: HashSet<&String> = req_output_owners.keys().collect();
        ensure!(outputs_spec == req_output_fkeys, "output keys mismatch");

        let ts = TaskState {
            task_id: Uuid::new_v4(),
            creator: requester,
            executor: req_executor,
            function_id: function.external_id(),
            function_owner: function.owner.clone(),
            function_arguments: req_func_args,
            inputs_ownership: req_input_owners,
            outputs_ownership: req_output_owners,
            participants,
            ..Default::default()
        };

        Ok(Task {
            state: ts,
            extra: Create,
        })
    }
}

impl Task<Assign> {
    pub fn new(ts: TaskState) -> Result<Self> {
        let task = Task::<Assign> {
            state: ts,
            extra: Assign,
        };
        Ok(task)
    }

    pub fn assign_input(
        &mut self,
        requester: &UserID,
        fname: &str,
        file: TeaclaveInputFile,
    ) -> Result<()> {
        ensure!(
            file.owner.contains(requester),
            "Assign: requester is not in the owner list. {:?}.",
            file.external_id()
        );

        self.state.inputs_ownership.check(fname, &file.owner)?;
        self.state.assigned_inputs.assign(fname, file)?;
        Ok(())
    }

    pub fn assign_output(
        &mut self,
        requester: &UserID,
        fname: &str,
        file: TeaclaveOutputFile,
    ) -> Result<()> {
        ensure!(
            file.owner.contains(requester),
            "Assign: requester is not in the owner list. {:?}.",
            file.external_id()
        );

        self.state.outputs_ownership.check(fname, &file.owner)?;
        self.state.assigned_outputs.assign(fname, file)?;
        Ok(())
    }
}

impl Task<Approve> {
    pub fn new(ts: TaskState) -> Result<Self> {
        let task = Task::<Approve> {
            state: ts,
            extra: Approve,
        };
        Ok(task)
    }

    pub fn approve(&mut self, requester: &UserID) -> Result<()> {
        ensure!(
            self.state.participants.contains(requester),
            "Unexpected user trying to approve a task: {:?}",
            requester
        );

        self.state.approved_users.insert(requester.clone());
        Ok(())
    }
}
impl Task<Stage> {
    pub fn new(ts: TaskState) -> Result<Self> {
        let task = Task::<Stage> {
            state: ts,
            extra: Stage,
        };
        Ok(task)
    }

    pub fn stage_for_running(
        &mut self,
        requester: &UserID,
        function: Function,
    ) -> Result<StagedTask> {
        ensure!(
            self.state.has_creator(&requester),
            "Requestor is not the task creater"
        );

        let function_arguments = self.state.function_arguments.clone();
        let staged_task = StagedTask {
            task_id: self.state.task_id,
            executor: self.state.executor,
            executor_type: function.executor_type,
            function_id: function.id,
            function_name: function.name,
            function_payload: function.payload,
            function_arguments,
            input_data: self.state.assigned_inputs.clone().into(),
            output_data: self.state.assigned_outputs.clone().into(),
        };
        Ok(staged_task)
    }
}

impl Task<Run> {
    pub fn new(ts: TaskState) -> Result<Self> {
        let task = Task::<Run> {
            state: ts,
            extra: Run,
        };
        Ok(task)
    }
}

impl Task<Finish> {
    pub fn new(ts: TaskState) -> Result<Self> {
        let task = Task::<Finish> {
            state: ts,
            extra: Finish,
        };
        Ok(task)
    }

    pub fn update_output_cmac(
        &mut self,
        fname: &str,
        auth_tag: &FileAuthTag,
    ) -> Result<&TeaclaveOutputFile> {
        self.state.assigned_outputs.update_cmac(fname, auth_tag)
    }

    pub fn update_result(&mut self, result: TaskResult) -> Result<()> {
        self.state.result = result;
        Ok(())
    }
}

impl Task<Done> {
    pub fn new(ts: TaskState) -> Result<Self> {
        let task = Task::<Done> {
            state: ts,
            extra: Done,
        };
        Ok(task)
    }
}

impl std::convert::TryFrom<Task<Assign>> for Task<Approve> {
    type Error = Error;
    fn try_from(task: Task<Assign>) -> Result<Task<Approve>> {
        ensure!(
            task.state.all_data_assigned(),
            "Not ready: Assign -> Approve"
        );
        Task::<Approve>::new(task.state)
    }
}

impl std::convert::TryFrom<Task<Approve>> for Task<Stage> {
    type Error = Error;
    fn try_from(task: Task<Approve>) -> Result<Task<Stage>> {
        ensure!(
            task.state.everyone_approved(),
            "Not ready: Apporve -> Stage"
        );
        Task::<Stage>::new(task.state)
    }
}

impl std::convert::TryFrom<Task<Stage>> for Task<Run> {
    type Error = Error;
    fn try_from(task: Task<Stage>) -> Result<Task<Run>> {
        Task::<Run>::new(task.state)
    }
}

impl std::convert::TryFrom<Task<Run>> for Task<Finish> {
    type Error = Error;
    fn try_from(task: Task<Run>) -> Result<Task<Finish>> {
        Task::<Finish>::new(task.state)
    }
}

impl std::convert::TryFrom<Task<Finish>> for Task<Done> {
    type Error = Error;
    fn try_from(task: Task<Finish>) -> Result<Task<Done>> {
        Task::<Done>::new(task.state)
    }
}

impl std::convert::TryFrom<TaskState> for Task<Assign> {
    type Error = Error;

    fn try_from(ts: TaskState) -> Result<Self> {
        let task = match ts.status {
            TaskStatus::Created => Task::<Assign>::new(ts)?,
            _ => bail!("Cannot restore to Assign from saved state "),
        };
        Ok(task)
    }
}

impl std::convert::TryFrom<TaskState> for Task<Approve> {
    type Error = Error;

    fn try_from(ts: TaskState) -> Result<Self> {
        let task = match ts.status {
            TaskStatus::Created => {
                let task: Task<Assign> = ts.try_into()?;
                task.try_into()?
            }
            TaskStatus::DataAssigned => Task::<Approve>::new(ts)?,
            _ => bail!("Cannot restore to Approve from saved state"),
        };
        Ok(task)
    }
}

impl std::convert::TryFrom<TaskState> for Task<Stage> {
    type Error = Error;

    fn try_from(ts: TaskState) -> Result<Self> {
        let task = match ts.status {
            TaskStatus::Created | TaskStatus::DataAssigned => {
                let task: Task<Approve> = ts.try_into()?;
                task.try_into()?
            }
            TaskStatus::Approved => Task::<Stage>::new(ts)?,
            _ => bail!("Cannot restore to Stage from saved state"),
        };
        Ok(task)
    }
}

impl std::convert::TryFrom<TaskState> for Task<Run> {
    type Error = Error;

    fn try_from(ts: TaskState) -> Result<Self> {
        let task = match ts.status {
            TaskStatus::Staged => Task::<Run>::new(ts)?,
            _ => bail!("Cannot restore to Run from saved state"),
        };
        Ok(task)
    }
}

impl std::convert::TryFrom<TaskState> for Task<Finish> {
    type Error = Error;

    fn try_from(ts: TaskState) -> Result<Self> {
        let task = match ts.status {
            TaskStatus::Running => Task::<Finish>::new(ts)?,
            _ => bail!("Cannot restore to Finish from saved state"),
        };
        Ok(task)
    }
}

impl std::convert::From<Task<Create>> for TaskState {
    fn from(mut task: Task<Create>) -> TaskState {
        task.state.status = TaskStatus::Created;
        task.state
    }
}

impl std::convert::From<Task<Assign>> for TaskState {
    fn from(mut task: Task<Assign>) -> TaskState {
        let nt: Result<Task<Approve>> = task.clone().try_into();
        match nt {
            Ok(mut t) => {
                t.state.status = t.extra.into();
                t.state
            }
            Err(_) => {
                task.state.status = task.extra.into();
                task.state
            }
        }
    }
}

impl std::convert::From<Task<Approve>> for TaskState {
    fn from(mut task: Task<Approve>) -> TaskState {
        let nt: Result<Task<Stage>> = task.clone().try_into();
        match nt {
            Ok(mut t) => {
                t.state.status = t.extra.into();
                t.state
            }
            Err(_) => {
                task.state.status = task.extra.into();
                task.state
            }
        }
    }
}

impl std::convert::From<Task<Stage>> for TaskState {
    fn from(mut task: Task<Stage>) -> TaskState {
        let nt: Result<Task<Run>> = task.clone().try_into();
        match nt {
            Ok(mut t) => {
                t.state.status = t.extra.into();
                t.state
            }
            Err(_) => {
                task.state.status = task.extra.into();
                task.state
            }
        }
    }
}

impl std::convert::From<Task<Run>> for TaskState {
    fn from(mut task: Task<Run>) -> TaskState {
        let nt: Result<Task<Finish>> = task.clone().try_into();
        match nt {
            Ok(mut t) => {
                t.state.status = t.extra.into();
                t.state
            }
            Err(_) => {
                task.state.status = task.extra.into();
                task.state
            }
        }
    }
}

impl std::convert::From<Task<Finish>> for TaskState {
    fn from(mut task: Task<Finish>) -> TaskState {
        let nt: Result<Task<Done>> = task.clone().try_into();
        match nt {
            Ok(mut t) => {
                t.state.status = t.extra.into();
                t.state
            }
            Err(_) => {
                task.state.status = task.extra.into();
                task.state
            }
        }
    }
}

/*
impl std::convert::From<Task<Done>> for TaskState {
    fn from(mut task: Task<Done>) -> TaskState {
        task.state.status = task.extra.into();
        task.state
    }
}
*/
