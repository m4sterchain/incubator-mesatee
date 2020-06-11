#!/bin/bash
set -e
REQUIRED_ENVS=("CMAKE_C_COMPILER" "CUR_PKG_NAME" "CUR_PKG_PATH"
"CUR_INSTALL_DIR" "TEACLAVE_OUT_DIR" "TEACLAVE_PROJECT_ROOT" "Service_Library_Name"
"SGX_COMMON_CFLAGS" "SGX_ENCLAVE_SIGNER" "SGX_LIBRARY_PATH" "TARGET" "Trts_Library_Name"
"TRUSTED_TARGET_DIR")
for var in "${REQUIRED_ENVS[@]}"; do
    [ -z "${!var}" ] && echo "Please set ${var}" && exit -1
done

if [ $# -eq 0 ]; then
    echo "Missing args: \$edl_lib_name."
    exit 1
fi
edl_lib_name="$1"

LIBENCLAVE_PATH="${TRUSTED_TARGET_DIR}/${TARGET}/lib${CUR_PKG_NAME}.a"
CONFIG_PATH="${TEACLAVE_PROJECT_ROOT}/${CUR_PKG_PATH}/Enclave.config.xml"
SIGNED_PATH="${CUR_INSTALL_DIR}/${CUR_PKG_NAME}.signed.so"
CUR_ENCLAVE_INFO_PATH="${TEACLAVE_OUT_DIR}/${CUR_PKG_NAME}_info.toml"

if [ ! "$LIBENCLAVE_PATH" -nt "$SIGNED_PATH" ] \
    && [ ! "$CONFIG_PATH" -nt "$SIGNED_PATH" ] \
    && [  ! "$SIGNED_PATH" -nt "$CUR_ENCLAVE_INFO_PATH" ]; then
    # "Skip linking ${SIGNED_PATH} because of no update."
    exit 0
fi

cd ${TEACLAVE_OUT_DIR}
echo "
${CMAKE_C_COMPILER} "lib${edl_lib_name}.o" -o \
    ${TEACLAVE_OUT_DIR}/${CUR_PKG_NAME}.so ${SGX_COMMON_CFLAGS} \
    -nostdlib -nodefaultlibs -nostartfiles \
    -L${SGX_LIBRARY_PATH} \
    -Wl,--whole-archive -l${Trts_Library_Name} \
    -Wl,--no-whole-archive -Wl,--start-group \
    -l${Service_Library_Name} \
    -lsgx_tkey_exchange \
    -lsgx_tprotected_fs \
    -lsgx_tstdc -lsgx_tcxx -lsgx_tservice -lsgx_tcrypto \
    -L${TEACLAVE_OUT_DIR} -lpycomponent ffi.o -lpypy-c -lsgx_tlibc_ext -lffi \
    -Wl,--end-group \
    -Wl,-Bstatic \
    -Wl,-Bsymbolic \
    -Wl,-pie,-eenclave_entry -Wl,--export-dynamic  \
    -Wl,--defsym,__ImageBase=0 \
    -Wl,--gc-sections \
    -Wl,--warn-common \
    -Wl,--version-script=${TEACLAVE_PROJECT_ROOT}/cmake/scripts/Enclave.lds
"
#${CMAKE_C_COMPILER} "lib${edl_lib_name}.o" -o \
#-L${TEACLAVE_OUT_DIR} -l${edl_lib_name} \
#    -lsgx_tprotected_fs \
#-Wl,--no-allow-shlib-undefined \
#-Wl,--no-undefined \
#${CMAKE_C_COMPILER} -o \
#-Wl,--warn-common \
${CMAKE_C_COMPILER} "lib${edl_lib_name}.o" -o \
    ${TEACLAVE_OUT_DIR}/${CUR_PKG_NAME}.so ${SGX_COMMON_CFLAGS} \
    -nostdlib -nodefaultlibs -nostartfiles \
    -L${SGX_LIBRARY_PATH} \
    -Wl,--whole-archive -l${Trts_Library_Name} \
    -Wl,--no-whole-archive -Wl,--start-group \
    -l${Service_Library_Name} \
    -lsgx_tkey_exchange \
    -lsgx_tprotected_fs \
    -lsgx_tstdc -lsgx_tcxx -lsgx_tservice -lsgx_tcrypto \
    -L${TEACLAVE_OUT_DIR} -lpycomponent ffi.o -lpypy-c -lsgx_tlibc_ext -lffi \
    -L${TRUSTED_TARGET_DIR}/${TARGET} -l${CUR_PKG_NAME} \
    -Wl,--end-group \
    -Wl,-Bstatic \
    -Wl,-Bsymbolic \
    -Wl,-pie,-eenclave_entry -Wl,--export-dynamic  \
    -Wl,--defsym,__ImageBase=0 \
    -Wl,--gc-sections \
    -Wl,--warn-common \
    -Wl,--version-script=${TEACLAVE_PROJECT_ROOT}/cmake/scripts/Enclave.lds
echo "
${SGX_ENCLAVE_SIGNER} sign -key ${TEACLAVE_PROJECT_ROOT}/keys/enclave_signing_key.pem \
    -enclave ${CUR_PKG_NAME}.so \
    -out ${CUR_INSTALL_DIR}/${CUR_PKG_NAME}.signed.so \
    -config ${TEACLAVE_PROJECT_ROOT}/${CUR_PKG_PATH}/Enclave.config.xml \
    -dumpfile ${CUR_PKG_NAME}.meta.txt > /dev/null 2>&1
"
${SGX_ENCLAVE_SIGNER} sign -key ${TEACLAVE_PROJECT_ROOT}/keys/enclave_signing_key.pem \
    -enclave ${CUR_PKG_NAME}.so \
    -out ${CUR_INSTALL_DIR}/${CUR_PKG_NAME}.signed.so \
    -config ${TEACLAVE_PROJECT_ROOT}/${CUR_PKG_PATH}/Enclave.config.xml \
    -dumpfile ${CUR_PKG_NAME}.meta.txt 
