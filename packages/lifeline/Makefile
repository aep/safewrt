all: lifeline

STRIP?=strip

.PHONY: install
install: lifeline
	$(STRIP) $^
	install -d $(DESTDIR)/usr/bin/
	install lifeline $(DESTDIR)/usr/bin/
	install -d $(DESTDIR)/etc/init.d/
	install init $(DESTDIR)/etc/init.d/lifeline
	install -d $(DESTDIR)/etc/rc.d/
	ln -sf /etc/init.d/lifeline $(DESTDIR)/etc/rc.d/S99lifeline


lifeline: lifeline.c
	$(CC) $^ -g -o $@ $(LDFLAGS)




