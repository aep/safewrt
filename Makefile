THIS=$(PWD)
OPENWRT_SRC=$(THIS)/openwrt
OUT=$(THIS)/out

export PATH := $(OUT)/host/bin/:$(OPENWRT_SRC)/staging_dir/host/bin:$(OPENWRT_SRC)/staging_dir/toolchain-mips_24kc_gcc-7.3.0_musl/bin/:$(PATH)

KERNEL= $(OUT)/target/kernel.uimage
ROOTIMG=$(OUT)/target/root.squashfs
FACTORY=$(OUT)/target/factory.img
GESIMG=$(OUT)/target/genesis.jffs2


all: outdirs $(GESIMG) $(FACTORY)
clean:
	rm -rf out

outdirs:
	@mkdir -p $(OUT)/tmp/
	@mkdir -p $(OUT)/target/
	@mkdir -p $(OUT)/host/bin/




########################## final images

$(FACTORY): $(KERNEL) $(ROOTIMG) $(GESIMG) $(OUT)/host/bin/tplink-safeloader
	@echo '~~~~~~> final/factory.img'
	tplink-safeloader  -B ARCHER-C7-V4 -V r6755+1-d089a5d773 -k $(KERNEL) -r $(ROOTIMG) -g $(GESIMG) -o $@ -j



########################## genesis
GESFS=$(OUT)/tmp/genesisfs/
GENPKG=$(OUT)/tmp/genesispkg/
GENBALL=$(OUT)/tmp/genesispkg.tar.xz
CONFIG=$(OUT)/tmp/config

GEN=$(THIS)/genesis/genesis-gf/target/mips-unknown-linux-musl/release/genesis-gf
$(GEN):
	@echo '~~~~~~> genesis/build'
	docker run -v $(THIS)/genesis/genesis-gf/:/src korhal/stasis-mips-rust:1.22.1 --release

$(GESFS): $(GEN) $(OUT)/host/bin/genesis-cli
	rm -rf $(GENPKG)
	rm -rf $(GESFS)
	mkdir -p $(GENPKG)
	cp $(GEN) $(GENPKG)/exe
	mips-openwrt-linux-strip $(GENPKG)/exe
	cd $(GENPKG) && tar cvzf $(GENBALL) .
	mkdir -p $(GESFS)
	hash="$$(genesis-cli hash $(GENBALL))" &&\
	mv $(GENBALL) $(GESFS)/$${hash} &&\
	ln -s $${hash} $(GESFS)/genesis
	echo '[wifi.ap.public]' > $(CONFIG)
	echo 'ssid = "Free Wifi"' >> $(CONFIG)
	hash="$$(genesis-cli hash $(CONFIG))" &&\
	mv $(CONFIG) $(GESFS)/$${hash} &&\
	ln -s $${hash} $(GESFS)/config

$(GESIMG): $(GESFS)
	@echo '~~~~~~> genesis/mksquashfs'
	mkfs.jffs2 --disable-compressor=zlib --root $(GESFS) --output $@ --squash --big-endian --pad

########################## kernel

cmdline="board=ARCHER-C7-V4 mtdparts=spi0.0:128k(factory-boot)ro,128k(fs-uboot)ro,10240k(firmware)ro,4864k(genesis),512(mac)ro,512(pin)ro,256(device-id)ro,64256(product-info)ro,704k(sysconf),64k(partition-table)ro,40k(support-list)ro,256(soft-version)ro,4k(extra-para)ro,1k(identity)ro,64k@0xff0000(art)ro console=ttyS0,115200"


$(KERNEL): $(OUT)/host/bin/patch-cmdline
	@echo '~~~~~~> kernel/patch'
	cp $(OPENWRT_SRC)/build_dir/target-mips_24kc_musl/linux-ar71xx_generic/vmlinux $(OUT)/tmp/vmlinuz
	patch-cmdline $(OUT)/tmp/vmlinuz $(cmdline)
	lzma e $(OUT)/tmp/vmlinuz  -lc1 -lp2 -pb2 $(OUT)/tmp/vmlinuz.lzma
	mkimage -A mips -O linux -T kernel -C lzma -a 0x80060000 -e 0x80060000 -n 'MIPS OpenWrt Linux-4.9.91' -d $(OUT)/tmp/vmlinuz.lzma $@


########################## rootfs
ROOTFS=$(OUT)/tmp/rootfs/

$(ROOTFS):
	@echo '~~~~~~> rootfs/openwrt'
	rsync -avz $(OPENWRT_SRC)/build_dir/target-mips_24kc_musl/root-ar71xx/ $@/
	rsync -avz $(THIS)/files/root/ $@/
	mkdir $@/genesis/



$(ROOTIMG): $(ROOTFS)
	@echo '~~~~~~> rootfs/mksquashfs4'
	mksquashfs4 $(ROOTFS) $@ -nopad -noappend -root-owned \
		-comp xz -Xpreset 9 -Xe -Xlc 0 -Xlp 2 -Xpb 2  -b 256k -p '/dev d 755 0 0' \
		-p '/dev/console c 600 0 0 5 1' \
		-processors 1 -fixed-time 1524559254



########################## tools

$(OUT)/host/bin/tplink-safeloader:  $(THIS)/tools/tplink-safeloader/tplink-safeloader.c $(THIS)/tools/tplink-safeloader/md5.c
	@echo '~~~~~~> tools/plink-safeloader'
	$(CC) $^ -o $@

$(OUT)/host/bin/patch-cmdline: $(THIS)/tools/patch-cmdline/patch-cmdline.c
	@echo '~~~~~~> tools/patch-cmdline'
	$(CC) $^ -o $@


$(OUT)/host/bin/genesis-cli: $(THIS)/tools/genesis-cli/src/main.rs $(THIS)/tools/genesis-cli/Cargo.toml
	@echo '~~~~~~> tools/genesis-cli'
	cd $(THIS)/tools/genesis-cli/ &&\
		cargo build --release  && \
		ln -s $(THIS)/tools/genesis-cli/target/release/genesis-cli $@

