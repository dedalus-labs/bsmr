# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Serve one persistent-worker request per call over the socket Bessemer assigns."""

import os
import sys
from concurrent import futures

import grpc
import worker_pb2
import worker_pb2_grpc


class Worker(worker_pb2_grpc.WorkerServicer):
    """Write each requested output and report it on standard error."""

    def Execute(
        self, request: worker_pb2.ExecuteCommand, context: grpc.ServicerContext
    ) -> worker_pb2.ExecuteResponse:
        """Create the single output path named by the request."""
        output = request.argv[0].decode()
        with open(output, "w") as file:
            file.write("written by worker\n")
        print(f"worker wrote {os.path.basename(output)}", file=sys.stderr, flush=True)
        return worker_pb2.ExecuteResponse(exit_code=0)


server = grpc.server(futures.ThreadPoolExecutor(max_workers=1))
worker_pb2_grpc.add_WorkerServicer_to_server(Worker(), server)
server.add_insecure_port("unix://" + os.environ["WORKER_SOCKET"])
server.start()
print("worker started", file=sys.stderr, flush=True)
server.wait_for_termination()
