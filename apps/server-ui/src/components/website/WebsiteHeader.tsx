import {
  Box,
  Burger,
  Divider,
  Drawer,
  Group,
  ScrollArea,
  Text
} from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import logo from '../../assets/logo.svg';
import classes from './WebsiteHeader.module.css';
import { useTranslation } from 'react-i18next';

export function WebsiteHeader() {
  const [drawerOpened, { toggle: toggleDrawer, close: closeDrawer }] = useDisclosure(false);
  const { t } = useTranslation();

  return (
    <Box pb={120}>
      <header className={classes.header}>
        <Group justify="space-between" h="100%">
          <Group gap="xs">
            <img className={`${classes.logo}`} src={logo}></img>
            <Text>{t('title')}</Text>
          </Group>
          <Group h="100%" gap={0} visibleFrom="sm">
            <a href="#" className={classes.link}>
              {t('menu.home')}
            </a>
            <Burger opened={drawerOpened} onClick={toggleDrawer} hiddenFrom="sm" />
          </Group>

          <Burger opened={drawerOpened} onClick={toggleDrawer} hiddenFrom="sm" />
        </Group>
      </header>

      <Drawer
        opened={drawerOpened}
        onClose={closeDrawer}
        size="100%"
        padding="md"
        title={t('navigator')}
        hiddenFrom="sm"
        zIndex={1000000}
      >
        <ScrollArea h="calc(100vh - 80px" mx="-md">
          <Divider my="sm" />
          <a href="#" className={classes.link}>
            {t('menu.home')}
          </a>
        </ScrollArea>
      </Drawer>
    </Box >
  );
}
