#!/usr/bin/env python3

#import http.server
#http.server.SimpleHTTPRequestHandler.extensions_map['.wasm'] = 'application/wasm'

import mimetypes
mimetypes.init()
mimetypes.add_type('application/wasm', '.wasm')

import runpy
runpy.run_module('http.server', run_name='__main__')
