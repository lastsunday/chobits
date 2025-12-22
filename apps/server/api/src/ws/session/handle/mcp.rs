use crate::mcp::client::device::DeviceMcpPhase;

impl<L> Session<L>
where
    L: Listener + Send,
{
    async fn handle_mcp(&mut self, message: &McpMessage) {
        let mcp_host = self.mcp_host.clone();
        let mut mcp_host = mcp_host.lock().await;
        if let Some(mcp_host) = mcp_host.as_mut() {
            match mcp_host.get_phase().await {
                DeviceMcpPhase::Initialize => {
                    self.handle_mcp_initialize_result(mcp_host, message).await;
                    self.request_mcp_tools_list(mcp_host).await;
                }
                DeviceMcpPhase::GetToolList => {
                    let has_next = self.handle_mcp_tools_list_result(mcp_host, message).await;
                    if has_next {
                        self.request_mcp_tools_list(mcp_host).await;
                    } else {
                        // TODO:end of get deivce mcp tools list
                        // let tools_list = mcp_host.get_all_tools().await;
                        // info!("{:?}", tools_list);
                    }
                }
            }
        } else {
            error!("mcp host is none");
        }
    }

    async fn request_mcp_initialize(
        &mut self,
        mcp_host: &mut UnionMcpHost,
        _hello_message: &HelloMessage,
    ) {
        let tx = self.output_tx.clone().unwrap();
        let request = mcp_host.create_initialize_request().await;
        // mcp request send
        let result = tx.send(Ok(FrameResult::McpResult(request))).await;
        if result.is_err() {
            info!("tx send mcp initialize reqeust failure");
        }
    }

    async fn handle_mcp_initialize_result(
        &mut self,
        mcp_host: &mut UnionMcpHost,
        message: &McpMessage,
    ) {
        mcp_host.handle_initialize_result(&message.payload).await;
    }

    async fn request_mcp_tools_list(&mut self, mcp_host: &mut UnionMcpHost) {
        let tx = self.output_tx.clone().unwrap();
        let result = tx
            .send(Ok(FrameResult::McpResult(
                mcp_host.create_tools_list_request().await,
            )))
            .await;
        if result.is_err() {
            info!("tx send mcp tools list reqeust failure");
        }
    }

    async fn handle_mcp_tools_list_result(
        &mut self,
        mcp_host: &mut UnionMcpHost,
        message: &McpMessage,
    ) -> bool {
        return mcp_host.handle_tools_list_result(&message.payload).await;
    }
}
