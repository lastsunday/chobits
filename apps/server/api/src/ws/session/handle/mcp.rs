use crate::mcp::client::device::{DeviceMcpClient, DeviceMcpPhase};

impl Session {
    async fn handle_mcp(&mut self, message: &McpMessage) {
        let mcp_host = self.mcp_host.clone();
        let mut mcp_host = mcp_host.lock().await;
        match mcp_host.get_phase().await {
            DeviceMcpPhase::Initialize => {
                self.handle_mcp_initialize_result(&mut mcp_host.device_mcp_client, message)
                    .await;
                self.request_mcp_tools_list(&mut mcp_host.device_mcp_client)
                    .await;
            }
            DeviceMcpPhase::GetToolList => {
                let has_next = self
                    .handle_mcp_tools_list_result(&mut mcp_host.device_mcp_client, message)
                    .await;
                if has_next {
                    self.request_mcp_tools_list(&mut mcp_host.device_mcp_client)
                        .await;
                } else {
                    // TODO:end of get deivce mcp tools list
                    // let tools_list = mcp_host.get_all_tools().await;
                    // info!("{:?}", tools_list);
                }
            }
        }
    }

    async fn request_mcp_initialize(
        &mut self,
        client: &mut DeviceMcpClient,
        _hello_message: &HelloMessage,
    ) {
        let tx = self.output_tx.clone().unwrap();
        let request = client.create_initialize_request().await;
        // mcp request send
        let result = tx.send(Ok(FrameResult::McpResult(request))).await;
        if result.is_err() {
            info!("tx send mcp initialize reqeust failure");
        }
    }

    async fn handle_mcp_initialize_result(
        &mut self,
        client: &mut DeviceMcpClient,
        message: &McpMessage,
    ) {
        client.handle_initialize_result(&message.payload).await;
    }

    async fn request_mcp_tools_list(&mut self, client: &mut DeviceMcpClient) {
        let tx = self.output_tx.clone().unwrap();
        let result = tx
            .send(Ok(FrameResult::McpResult(
                client.create_tools_list_request().await,
            )))
            .await;
        if result.is_err() {
            info!("tx send mcp tools list reqeust failure");
        }
    }

    async fn handle_mcp_tools_list_result(
        &mut self,
        client: &mut DeviceMcpClient,
        message: &McpMessage,
    ) -> bool {
        return client.handle_tools_list_result(&message.payload).await;
    }
}
