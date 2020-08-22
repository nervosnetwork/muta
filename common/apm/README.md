# Metrics documentation for promethues
All current metrics and usage
## API

| Metric name | Metric types | Related Grafana panel |
|---|---|---|
| muta_api_request_total             | counter      |                          |
| muta_api_request_result_total      | counter      | processed_tx_request     |
| muta_api_request_time_cost_seconds | histogram    |                          |


## Consensus
<table>
<thead>
  <tr>
    <th colspan="3">Consensus</th>
  </tr>
</thead>
<tbody>
  <tr>
    <td>Metric name</td>
    <td>Metric types</td>
    <td>Related Grafana panel</td>
  </tr>
  <tr>
    <td>muta_concensus_result</td>
    <td>counter</td>
    <td></td>
  </tr>
  <tr>
    <td>muta_consensus_time_cost_seconds</td>
    <td>histogram</td>
    <td>exec_p90</td>
  </tr>
  <tr>
    <td>muta_consensus_round</td>
    <td>gauge</td>
    <td>consensus_round_cost</td>
  </tr>
  <tr>
    <td>muta_executing_queue</td>
    <td>gauge</td>
    <td>executing_block_size</td>
  </tr>
  <tr>
    <td rowspan="3">muta_consensus_height</td>
    <td rowspan="3">gauge</td>
    <td>get_cf_each_block_time_usage</td>
  </tr>
  <tr>
    <td>put_cf_each_block_time_usage</td>
  </tr>
  <tr>
    <td>current_height</td>
  </tr>
  <tr>
    <td>muta_consensus_committed_tx_total</td>
    <td>counter</td>
    <td>TPS</td>
  </tr>
  <tr>
    <td>muta_consensus_sync_block_duration</td>
    <td>counter</td>
    <td>synced_block</td>
  </tr>
  <tr>
    <td>muta_consensus_duration_seconds</td>
    <td>histogram</td>
    <td>consensus_p90</td>
  </tr>
</tbody>
</table>

## Mempool		
<table>
<thead>
  <tr>
    <th>Metric name</th>
    <th>Metric types</th>
    <th>Related Grafana panel</th>
  </tr>
</thead>
<tbody>
  <tr>
    <td>muta_mempool_counter</td>
    <td>counter</td>
    <td></td>
  </tr>
  <tr>
    <td>muta_mempool_result_counter</td>
    <td>counter</td>
    <td></td>
  </tr>
  <tr>
    <td>muta_mempool_cost_seconds</td>
    <td>histogram</td>
    <td></td>
  </tr>
  <tr>
    <td>muta_mempool_package_size_vec</td>
    <td>histogram</td>
    <td></td>
  </tr>
  <tr>
    <td>muta_mempool_current_size_vec</td>
    <td>histogram</td>
    <td></td>
  </tr>
  <tr>
    <td>muta_mempool_tx_count</td>
    <td>guage</td>
    <td>mempool_cached_tx</td>
  </tr>
</tbody>
</table>

## Network		
<table>
<thead>
  <tr>
    <th>Metric name</th>
    <th>Metric types</th>
    <th>Related Grafana panel</th>
  </tr>
</thead>
<tbody>
  <tr>
    <td>muta_network_message_total</td>
    <td>counter</td>
    <td>network_message_arrival_rate</td>
  </tr>
  <tr>
    <td>muta_network_rpc_result_total</td>
    <td>counter</td>
    <td></td>
  </tr>
  <tr>
    <td>muta_network_protocol_time_cost_seconds</td>
    <td>histogram</td>
    <td></td>
  </tr>
  <tr>
    <td>muta_network_total_pending_data_size</td>
    <td>gauge</td>
    <td></td>
  </tr>
  <tr>
    <td>muta_network_ip_pending_data_size</td>
    <td>gauge</td>
    <td></td>
  </tr>
  <tr>
    <td>muta_network_received_message_in_processing_guage</td>
    <td>gauge</td>
    <td>Received messages in processing</td>
  </tr>
  <tr>
    <td>muta_network_received_ip_message_in_processing_guage</td>
    <td>gauge</td>
    <td>Received messages in processing by ip</td>
  </tr>
  <tr>
    <td>muta_network_connected_peers</td>
    <td>gauge</td>
    <td>Connected Peers</td>
  </tr>
  <tr>
    <td rowspan="2">muta_network_ip_ping_in_ms</td>
    <td rowspan="2">gauge</td>
    <td>Ping (ms)</td>
  </tr>
  <tr>
    <td>Ping by ip</td>
  </tr>
  <tr>
    <td>muta_network_ip_disconnected_count</td>
    <td>counter</td>
    <td>Disconnected count(To other peers)</td>
  </tr>
  <tr>
    <td>muta_network_outbound_connecting_peers</td>
    <td>gauge</td>
    <td>Connecting Peers</td>
  </tr>
  <tr>
    <td>muta_network_unidentified_connections</td>
    <td>gauge</td>
    <td>Received messages in processing</td>
  </tr>
  <tr>
    <td>muta_network_saved_peer_count</td>
    <td>counter</td>
    <td>Saved peers</td>
  </tr>
  <tr>
    <td>muta_network_tagged_consensus_peers</td>
    <td>gauge</td>
    <td>Consensus peers</td>
  </tr>
  <tr>
    <td>muta_network_connected_consensus_peers</td>
    <td>gauge</td>
    <td>Connected Consensus Peers (Minus itself)</td>
  </tr>
</tbody>
</table>

## Storage
<table>
<thead>
  <tr>
    <th>Metric name</th>
    <th>Metric types</th>
    <th>Related Grafana panel</th>
  </tr>
</thead>
<tbody>
  <tr>
    <td>muta_storage_put_cf_seconds</td>
    <td>counter</td>
    <td>put_cf_each_block_time_usage</td>
  </tr>
  <tr>
    <td>muta_storage_put_cf_bytes</td>
    <td>counter</td>
    <td></td>
  </tr>
  <tr>
    <td>muta_storage_get_cf_seconds</td>
    <td>counter</td>
    <td>get_cf_each_block_time_usage</td>
  </tr>
  <tr>
    <td>muta_storage_get_cf_total</td>
    <td>counter</td>
    <td></td>
  </tr>
</tbody>
</table>